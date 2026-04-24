//! LLM-based speaker-name guessing.
//!
//! Builds a compact `Speaker N: <text>` transcript and asks the user's
//! configured summary model for a JSON map of speaker indices to likely
//! first names. Silent fallback (returns empty) if no model is configured
//! or the provider call fails — the cue parser + adapter passes still run.
//!
//! Dispatches through `summary::llm_client::generate_summary` so the call
//! inherits every provider the summary pipeline supports (OpenAI / Claude /
//! Groq / Ollama / OpenRouter / CustomOpenAI / BuiltInAI).

use std::collections::HashMap;

use reqwest::Client;
use sqlx::SqlitePool;
use tauri::Manager;

use crate::database::models::{Speaker, Transcript};
use crate::database::repositories::setting::SettingsRepository;
use crate::summary::llm_client::{generate_summary, LLMProvider};

use super::cue_parser::Candidate;

/// Build the speaker-labeled transcript text that the LLM would see. Kept
/// public because the same projection is useful for "copy with speakers"
/// affordances on the frontend.
pub fn build_labeled_transcript(
    speakers: &[Speaker],
    speaker_id_to_cluster: &HashMap<String, i64>,
    transcripts: &[Transcript],
) -> String {
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

/// Short, JSON-strict prompt designed to survive small local models.
pub fn build_prompt(labeled_transcript: &str, cluster_count: usize) -> String {
    format!(
        "You are given a meeting transcript where speakers are labeled as \
         Speaker 1, Speaker 2, etc. Based on how they are addressed by each \
         other (vocatives, names), infer their likely first name. Respond \
         ONLY with a JSON object of the shape \
         {{\"0\": \"Alice\", \"1\": \"Bob\"}} where keys are speaker indices \
         (0-based, matching Speaker 1 = \"0\", Speaker 2 = \"1\") and values \
         are first names you are reasonably confident about. Omit speakers \
         whose name you cannot infer. There are {cluster_count} speakers in \
         this transcript. Do not add commentary, prose, or markdown fences.\n\n\
         Transcript:\n{labeled_transcript}"
    )
}

/// Parse the LLM response into (cluster_idx, name) pairs. The raw output is
/// noisy in practice — local Ollama models sometimes wrap the JSON in
/// prose or markdown fences. We search for the first `{...}` block and try
/// to parse it.
fn parse_response(raw: &str) -> HashMap<i64, String> {
    let start = raw.find('{');
    let end = raw.rfind('}');
    let slice = match (start, end) {
        (Some(s), Some(e)) if e > s => &raw[s..=e],
        _ => return HashMap::new(),
    };
    let Ok(parsed): Result<HashMap<String, String>, _> = serde_json::from_str(slice) else {
        return HashMap::new();
    };
    parsed
        .into_iter()
        .filter_map(|(k, v)| {
            let idx: i64 = k.trim().parse().ok()?;
            let name = v.trim();
            if name.is_empty() {
                return None;
            }
            Some((idx, name.to_string()))
        })
        .collect()
}

/// Fetch provider config, dispatch the prompt, parse the response.
/// Returns an empty Vec for any failure mode so the caller's name
/// pipeline always completes.
pub async fn extract_candidates<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    pool: &SqlitePool,
    speakers: &[Speaker],
    speaker_id_to_cluster: &HashMap<String, i64>,
    transcripts: &[Transcript],
) -> Vec<Candidate> {
    if speakers.is_empty() || transcripts.is_empty() {
        return Vec::new();
    }

    // Model config lives in the settings table; no config → silent skip.
    let Ok(Some(config)) = SettingsRepository::get_model_config(pool).await else {
        log::info!("llm_namer: no model config configured, skipping LLM pass");
        return Vec::new();
    };

    let Ok(provider) = LLMProvider::from_str(&config.provider) else {
        log::warn!(
            "llm_namer: unknown provider '{}', skipping",
            &config.provider
        );
        return Vec::new();
    };

    let api_key = SettingsRepository::get_api_key(pool, &config.provider)
        .await
        .ok()
        .flatten()
        .unwrap_or_default();

    // BuiltInAI needs the app data dir for sidecar weights.
    let app_data_dir = app
        .path()
        .app_data_dir()
        .ok();

    let labeled = build_labeled_transcript(speakers, speaker_id_to_cluster, transcripts);
    let user_prompt = build_prompt(&labeled, speakers.len());
    let system_prompt =
        "You are a careful meeting-notes assistant that outputs only strict JSON when asked.";

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .unwrap_or_else(|_| Client::new());

    let raw = match generate_summary(
        &client,
        &provider,
        &config.model,
        &api_key,
        system_prompt,
        &user_prompt,
        config.ollama_endpoint.as_deref(),
        None,
        Some(512),
        Some(0.1),
        Some(0.9),
        app_data_dir.as_ref(),
        None,
    )
    .await
    {
        Ok(text) => text,
        Err(e) => {
            log::warn!("llm_namer: provider call failed, skipping: {}", e);
            return Vec::new();
        }
    };

    let map = parse_response(&raw);
    if map.is_empty() {
        log::info!(
            "llm_namer: provider returned no parseable candidates (first 200 chars: {})",
            raw.chars().take(200).collect::<String>()
        );
        return Vec::new();
    }

    map.into_iter()
        .map(|(cluster_idx, name)| Candidate {
            cluster_idx,
            name,
            confidence: 0.82,
        })
        .collect()
}
