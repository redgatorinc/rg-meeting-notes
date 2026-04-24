//! LLM-based speaker-name guessing.
//!
//! Builds a compact `Speaker N: <text>` transcript and asks the user's
//! configured summary model for a JSON map
//! `{ "0": "Fabio", "1": "Ana", ... }` alongside per-guess confidence.
//! Returns no candidates if no model is configured — silent fallback, the
//! cue parser and adapter paths still run.
//!
//! Phase-1 scope: the wire is in place and the prompt is shaped, but the
//! actual summary-model call is deferred to a follow-up PR because the
//! summary engine has per-provider plumbing (Ollama / Claude / Groq /
//! OpenRouter / BuiltIn) and wiring all of them here is a project in
//! itself. For now we log the prompt and return an empty vector — the UI
//! approval panel just won't show "llm" candidates.
//!
//! When the real call lands, it should go through `summary::summary_engine`
//! so it reuses the same model config + API key management the summary
//! pipeline already has.

use std::collections::HashMap;

use crate::database::models::{Speaker, Transcript};

use super::cue_parser::Candidate;

/// Build the speaker-labeled transcript text that the LLM would see. Kept
/// public because the same projection is useful for "copy with speakers"
/// affordances on the frontend.
pub fn build_labeled_transcript(
    speakers: &[Speaker],
    speaker_id_to_cluster: &HashMap<String, i64>,
    transcripts: &[Transcript],
) -> String {
    // Map cluster_idx -> display label
    let mut label_for_cluster: HashMap<i64, String> = HashMap::new();
    for s in speakers {
        label_for_cluster
            .entry(s.cluster_idx)
            .or_insert_with(|| format!("Speaker {}", s.cluster_idx + 1));
    }

    let mut out = String::with_capacity(transcripts.len() * 64);
    let mut last_cluster: Option<i64> = None;
    for t in transcripts {
        let cluster = t
            .speaker_id
            .as_ref()
            .and_then(|sid| speaker_id_to_cluster.get(sid))
            .copied();
        if let Some(c) = cluster {
            if last_cluster != Some(c) {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(label_for_cluster.get(&c).map(|s| s.as_str()).unwrap_or("?"));
                out.push_str(": ");
                last_cluster = Some(c);
            } else {
                out.push(' ');
            }
        } else if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(t.transcript.trim());
    }
    out
}

/// Build the prompt body sent to the model. Intentionally short and
/// JSON-strict so local Ollama models (which struggle with long
/// instructions) still produce parseable output.
pub fn build_prompt(labeled_transcript: &str, cluster_count: usize) -> String {
    format!(
        "You are given a meeting transcript where speakers are labeled \
         as Speaker 1, Speaker 2, etc. Based on how they are addressed \
         by each other (vocatives, names), infer their likely first \
         name. Respond ONLY with a JSON object of the shape \
         {{\"0\": \"Alice\", \"1\": \"Bob\"}} where keys are speaker \
         indices (0-based) and values are first names you are confident \
         about. Omit speakers whose name you cannot infer. There are \
         {cluster_count} speakers in this transcript.\n\n\
         Transcript:\n{labeled_transcript}"
    )
}

/// Stub: prompt is built and logged, but the provider call isn't wired
/// yet. Returns empty so the pipeline still completes.
pub async fn extract_candidates(
    speakers: &[Speaker],
    speaker_id_to_cluster: &HashMap<String, i64>,
    transcripts: &[Transcript],
) -> Vec<Candidate> {
    if speakers.is_empty() || transcripts.is_empty() {
        return Vec::new();
    }
    let labeled = build_labeled_transcript(speakers, speaker_id_to_cluster, transcripts);
    let _prompt = build_prompt(&labeled, speakers.len());
    log::info!(
        "llm_namer: prompt built ({} chars, {} speakers) — provider call deferred to follow-up PR",
        labeled.len(),
        speakers.len()
    );
    Vec::new()
}
