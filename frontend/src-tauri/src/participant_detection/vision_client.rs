//! Call an OpenAI-compatible vision endpoint with a captured PNG and ask
//! for `{participants, current_speaker, confidence}`.
//!
//! Phase 1 supports any OpenAI-compatible `/chat/completions` server that
//! accepts the `image_url` content part (OpenAI, Azure OpenAI, LiteLLM,
//! vLLM, Ollama's `/v1/chat/completions`, etc). Claude can be added later
//! with its own request shape.

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use url::Url;

use super::{DetectionResult, Participant};

/// Minimal shape the vision LLM must return. We ask for strict JSON and
/// strip any ```json fences before parsing.
#[derive(Debug, Deserialize)]
struct RawDetection {
    #[serde(default)]
    participants: Vec<String>,
    #[serde(default)]
    current_speaker: Option<String>,
    #[serde(default)]
    confidence: f32,
}

const SYSTEM_PROMPT: &str = "You identify participants in a video meeting screenshot. Return ONLY valid JSON, no markdown, no prose, no code fences. Shape: {\"participants\": [\"string\", …], \"current_speaker\": \"string|null\", \"confidence\": 0.0-1.0}. Use the display names shown on each tile. `current_speaker` is the one currently speaking (mic-ring, bold border, speaking pulse). null if unclear. If the image does not show a meeting, return {\"participants\": [], \"current_speaker\": null, \"confidence\": 0}.";

const USER_PROMPT: &str = "Who is in this meeting, and who is currently speaking?";

pub struct VisionProvider {
    /// Full URL of the chat/completions endpoint (e.g.
    /// `https://api.openai.com/v1/chat/completions`).
    pub endpoint: String,
    pub api_key: Option<String>,
    /// e.g. `gpt-4o-mini`.
    pub model: String,
}

pub async fn detect_participants(
    provider: &VisionProvider,
    png_bytes: &[u8],
    source_app: &str,
) -> Result<DetectionResult> {
    if !provider.endpoint.starts_with("http://") && !provider.endpoint.starts_with("https://") {
        return Err(anyhow!(
            "Vision endpoint must start with http:// or https:// (got {})",
            provider.endpoint
        ));
    }

    let host = Url::parse(&provider.endpoint)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string());

    let data_url = format!("data:image/png;base64,{}", B64.encode(png_bytes));

    let body = serde_json::json!({
        "model": provider.model,
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            {
                "role": "user",
                "content": [
                    { "type": "text", "text": USER_PROMPT },
                    { "type": "image_url", "image_url": { "url": data_url, "detail": "low" } }
                ]
            }
        ],
        "temperature": 0.0,
        "response_format": { "type": "json_object" }
    });

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("Failed to build HTTP client")?;

    let mut req = client
        .post(&provider.endpoint)
        .header("Content-Type", "application/json")
        .json(&body);
    if let Some(key) = provider.api_key.as_deref().filter(|k| !k.trim().is_empty()) {
        req = req.header("Authorization", format!("Bearer {}", key));
    }

    let resp = req
        .send()
        .await
        .map_err(|e| anyhow!("Vision API request failed: {}", e))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(anyhow!(
            "Vision API returned {} — {}",
            status,
            truncate(&text, 500)
        ));
    }

    // Extract the model's `content` field, then parse as our JSON shape.
    let envelope: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| anyhow!("Vision API response is not JSON: {} — body: {}", e, truncate(&text, 500)))?;

    let content = envelope
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            anyhow!(
                "Vision API response missing /choices/0/message/content — body: {}",
                truncate(&text, 500)
            )
        })?;

    let cleaned = strip_json_fences(content);

    let raw: RawDetection = serde_json::from_str(cleaned).map_err(|e| {
        anyhow!(
            "Vision model did not return parseable JSON: {} — content: {}",
            e,
            truncate(cleaned, 500)
        )
    })?;

    let participants = raw
        .participants
        .into_iter()
        .filter_map(|name| {
            let trimmed = name.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(Participant { name: trimmed })
            }
        })
        .collect::<Vec<_>>();

    Ok(DetectionResult {
        participants,
        current_speaker: raw
            .current_speaker
            .and_then(|s| {
                let trimmed = s.trim().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            }),
        confidence: raw.confidence.clamp(0.0, 1.0),
        provider_host: host,
        source_app: source_app.to_string(),
    })
}

/// Remove a leading ```json or ``` and trailing ``` the model may have
/// wrapped around otherwise-valid JSON.
fn strip_json_fences(s: &str) -> &str {
    let trimmed = s.trim();
    let no_prefix = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed)
        .trim_start();
    no_prefix.strip_suffix("```").unwrap_or(no_prefix).trim()
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        // Safe-truncate on char boundary.
        let mut idx = max;
        while !s.is_char_boundary(idx) && idx > 0 {
            idx -= 1;
        }
        &s[..idx]
    }
}
