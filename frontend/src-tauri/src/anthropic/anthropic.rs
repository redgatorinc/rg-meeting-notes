use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use std::time::{Duration, Instant};
use tauri::command;

/// Anthropic (Claude) model information returned to frontend
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnthropicModel {
    pub id: String,
    pub display_name: Option<String>,
}

/// API response model from Anthropic
#[derive(Debug, Deserialize)]
struct AnthropicApiModel {
    id: String,
    display_name: Option<String>,
    #[allow(dead_code)]
    created_at: Option<String>,
}

/// API response wrapper from Anthropic
#[derive(Debug, Deserialize)]
struct AnthropicApiResponse {
    data: Vec<AnthropicApiModel>,
    #[serde(default)]
    has_more: bool,
    #[serde(default)]
    last_id: Option<String>,
}

/// Cache entry for models
struct CacheEntry {
    models: Vec<AnthropicModel>,
    fetched_at: Instant,
}

/// Global cache for Anthropic models (5 minute TTL)
static MODELS_CACHE: RwLock<Option<CacheEntry>> = RwLock::new(None);

/// Cache TTL in seconds
const CACHE_TTL_SECS: u64 = 300;

/// Fallback models when API fetch fails (matches frontend hardcoded values)
const FALLBACK_MODELS: &[(&str, &str)] = &[
    ("claude-sonnet-4-5-20250929", "Claude 4.5 Sonnet"),
    ("claude-haiku-4-5-20251001", "Claude 4.5 Haiku"),
    ("claude-opus-4-1-20250805", "Claude 4.1 Opus"),
    ("claude-sonnet-4-20250514", "Claude 4 Sonnet"),
];

/// Get fallback models as AnthropicModel vec
fn get_fallback_models() -> Vec<AnthropicModel> {
    FALLBACK_MODELS
        .iter()
        .map(|(id, name)| AnthropicModel {
            id: id.to_string(),
            display_name: Some(name.to_string()),
        })
        .collect()
}

/// Check if model is a chat-capable model
fn is_chat_model(model_id: &str) -> bool {
    let id = model_id.to_lowercase();
    // Include Claude models only
    id.starts_with("claude-")
}

/// Fetch Anthropic models from API
///
/// # Arguments
/// * `api_key` - Anthropic API key
///
/// # Returns
/// Vector of available models, or fallback models on error
#[command]
pub async fn get_anthropic_models(api_key: Option<String>) -> Result<Vec<AnthropicModel>, String> {
    // Return fallback if no API key provided
    let api_key = match api_key {
        Some(key) if !key.trim().is_empty() => key.trim().to_string(),
        _ => {
            log::info!("No Anthropic API key provided, returning fallback models");
            return Ok(get_fallback_models());
        }
    };

    // Check cache first
    {
        let cache = MODELS_CACHE.read().map_err(|e| e.to_string())?;
        if let Some(entry) = cache.as_ref() {
            if entry.fetched_at.elapsed() < Duration::from_secs(CACHE_TTL_SECS) {
                log::info!(
                    "Returning cached Anthropic models ({} models)",
                    entry.models.len()
                );
                return Ok(entry.models.clone());
            }
        }
    }

    // Fetch from API (paginated — Anthropic returns up to 1000 per page via ?limit=1000
    // and signals more pages via has_more + last_id).
    log::info!("Fetching Anthropic models from API (paginated)...");
    let client = reqwest::Client::new();
    let mut all_models: Vec<AnthropicModel> = Vec::new();
    let mut after_id: Option<String> = None;

    loop {
        let mut url = String::from("https://api.anthropic.com/v1/models?limit=1000");
        if let Some(ref id) = after_id {
            url.push_str(&format!("&after_id={}", id));
        }

        let response = match client
            .get(&url)
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .timeout(Duration::from_secs(10))
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                log::warn!("Failed to fetch Anthropic models: {}. Using fallback.", e);
                return Ok(get_fallback_models());
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            log::warn!(
                "Anthropic API returned status {}: {}. Using fallback models.",
                status,
                body
            );
            return Ok(get_fallback_models());
        }

        let api_response: AnthropicApiResponse = match response.json().await {
            Ok(data) => data,
            Err(e) => {
                log::warn!("Failed to parse Anthropic response: {}. Using fallback.", e);
                return Ok(get_fallback_models());
            }
        };

        let has_more = api_response.has_more;
        let next_after = api_response.last_id.clone();

        for m in api_response.data.into_iter().filter(|m| is_chat_model(&m.id)) {
            all_models.push(AnthropicModel {
                id: m.id,
                display_name: m.display_name,
            });
        }

        if !has_more || next_after.is_none() {
            break;
        }
        after_id = next_after;
    }

    let models = all_models;

    // If no models returned, use fallback
    if models.is_empty() {
        log::warn!("No chat models returned from Anthropic API. Using fallback.");
        return Ok(get_fallback_models());
    }

    log::info!("Fetched {} Anthropic models from API", models.len());

    // Update cache
    {
        let mut cache = MODELS_CACHE.write().map_err(|e| e.to_string())?;
        *cache = Some(CacheEntry {
            models: models.clone(),
            fetched_at: Instant::now(),
        });
    }

    Ok(models)
}

/// Clear the models cache (useful when API key changes)
pub fn clear_cache() {
    if let Ok(mut cache) = MODELS_CACHE.write() {
        *cache = None;
        log::info!("Anthropic models cache cleared");
    }
}
