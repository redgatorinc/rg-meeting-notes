//! Pure-Rust vocative-name extractor.
//!
//! Finds likely first-name mentions in the transcript and attributes them
//! to the speaker cluster most likely being addressed (usually the
//! cluster *before* the mention). Zero external deps — runs offline on
//! any transcript.
//!
//! Patterns we recognise (case-insensitive prefix):
//!
//!   - `Hey <Name>,`                 → address    — next turn is the named person
//!   - `, <Name>`  / `? <Name>`      → vocative  — previous turn addressed them
//!   - `Thanks <Name>` / `Thank you <Name>` → addressed
//!   - `<Name>, what do you think`   → leading vocative
//!
//! A hit is recorded as a candidate against the addressed cluster with a
//! flat confidence of 0.6 (we're guessing a name from context; this
//! isn't speech recognition). The LLM pass in `llm_namer` produces higher-
//! confidence candidates when a model is configured; the adapter pass in
//! `adapter_names` produces the highest when the integrated adapter
//! returns a matching participant count.

use std::collections::HashMap;

use crate::database::models::Transcript;

#[derive(Debug, Clone)]
pub struct Candidate {
    pub cluster_idx: i64,
    pub name: String,
    pub confidence: f32,
}

/// Names that look like first names but almost always aren't — small
/// English blacklist to cut false positives. Lowercase compare.
const BLACKLIST: &[&str] = &[
    "I", "A", "An", "The", "My", "Our", "His", "Her", "Their", "Your",
    "Ok", "Okay", "Alright", "Cool", "Yeah", "Yes", "No", "Maybe", "Sure",
    "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday",
    "January", "February", "March", "April", "May", "June", "July",
    "August", "September", "October", "November", "December",
    "Google", "Teams", "Zoom", "Slack", "Discord", "Microsoft", "Apple",
    "Mac", "PC", "AI", "CEO", "CTO", "CFO", "VP", "USA", "EU", "UK",
];

fn is_plausible_name(word: &str) -> bool {
    if word.len() < 2 || word.len() > 24 {
        return false;
    }
    if !word.chars().next().map(|c| c.is_ascii_uppercase()).unwrap_or(false) {
        return false;
    }
    if !word
        .chars()
        .skip(1)
        .all(|c| c.is_ascii_alphabetic() || c == '\'' || c == '-')
    {
        return false;
    }
    let lower = word.to_lowercase();
    !BLACKLIST.iter().any(|b| b.to_lowercase() == lower)
}

fn strip_punct(word: &str) -> &str {
    let bytes = word.as_bytes();
    let mut start = 0;
    let mut end = bytes.len();
    while start < end && !bytes[start].is_ascii_alphabetic() {
        start += 1;
    }
    while end > start && !bytes[end - 1].is_ascii_alphabetic() && bytes[end - 1] != b'\'' {
        end -= 1;
    }
    &word[start..end]
}

/// Scan one transcript line; return a list of addressed-name candidates
/// (most lines produce zero). The caller maps them onto a speaker cluster.
fn extract_names_from_text(text: &str) -> Vec<String> {
    let mut hits: Vec<String> = Vec::new();
    let tokens: Vec<&str> = text.split_whitespace().collect();

    for i in 0..tokens.len() {
        let t = tokens[i];
        let prev = if i > 0 { Some(tokens[i - 1]) } else { None };
        let word = strip_punct(t);
        if !is_plausible_name(word) {
            continue;
        }
        let trigger = match prev.map(|p| p.to_lowercase()) {
            Some(p) if p == "hey" || p == "hi" || p == "hello" || p == "ok" => true,
            Some(p) if p == "thanks" || p == "thank" => true,
            Some(p) if p == "right" || p == "yeah" || p == "yes" => true,
            _ => false,
        };
        let vocative = t.ends_with(',') || t.ends_with('?') || t.ends_with('.');
        if trigger || vocative {
            hits.push(word.to_string());
        }
    }
    hits
}

/// Main entry point. Assumes transcripts are already annotated with
/// `speaker_id` that points at `speakers.id` (post-diarize). Name-hit
/// counts are aggregated per cluster and scaled by the number of mentions
/// — more mentions → slightly higher confidence, capped at 0.9.
pub fn extract_candidates(
    transcripts: &[Transcript],
    speaker_id_to_cluster: &HashMap<String, i64>,
) -> Vec<Candidate> {
    // (cluster_idx, name_lowercase) -> (mention_count, original_casing)
    let mut counts: HashMap<(i64, String), (u32, String)> = HashMap::new();

    // Pair each transcript line with its cluster_idx. A name mentioned in
    // speaker A's line is most likely addressed to the PREVIOUS non-same-
    // cluster speaker — hence `prev_other_cluster`.
    let mut prev_other_cluster: Option<i64> = None;
    let mut current_cluster: Option<i64> = None;

    for t in transcripts {
        let cluster = t
            .speaker_id
            .as_ref()
            .and_then(|sid| speaker_id_to_cluster.get(sid))
            .copied();

        if let Some(c) = cluster {
            if current_cluster != Some(c) {
                if current_cluster.is_some() {
                    prev_other_cluster = current_cluster;
                }
                current_cluster = Some(c);
            }
        }

        let names = extract_names_from_text(&t.transcript);
        for name in names {
            let Some(target) = prev_other_cluster.or(current_cluster) else {
                continue;
            };
            let key = (target, name.to_lowercase());
            let entry = counts
                .entry(key)
                .or_insert_with(|| (0, name.clone()));
            entry.0 += 1;
        }
    }

    counts
        .into_iter()
        .map(|((cluster_idx, _), (mentions, original))| {
            let conf = (0.55 + (mentions as f32 - 1.0) * 0.1).min(0.9);
            Candidate {
                cluster_idx,
                name: original,
                confidence: conf,
            }
        })
        .collect()
}
