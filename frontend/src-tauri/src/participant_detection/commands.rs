//! Tauri commands for AI participant detection (Phase 1 MVP).
//!
//! One public command — `participant_detect_snapshot(meeting_id)`:
//!   1. Capture the active meeting app's window (PNG, via xcap).
//!   2. Send the PNG to a vision-capable OpenAI-compatible endpoint.
//!   3. Parse `{participants, current_speaker, confidence}`.
//!   4. If there is exactly one speaker cluster without a `display_name`
//!      set and the model returned a `current_speaker`, auto-rename that
//!      cluster. Otherwise leave renaming to the UI (modal-assisted).
//!
//! Consent: a second command `participant_consent_set` stores the user's
//! acceptance in the existing settings KV so the frontend can gate the
//! call behind a one-time modal.

use std::env;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_store::StoreExt;

use super::{vision_client, window_capture, DetectionResult};
use crate::database::repositories::setting::SettingsRepository;
use crate::database::repositories::speaker::SpeakersRepository;
use crate::state::AppState;

const CONSENT_STORE: &str = "store.json";
const CONSENT_KEY: &str = "participant_detection_consent";

#[derive(Debug, Serialize, Deserialize)]
pub struct ParticipantConsent {
    pub enabled: bool,
}

fn read_consent<R: Runtime>(app: &AppHandle<R>) -> bool {
    let Ok(store) = app.store(CONSENT_STORE) else {
        return false;
    };
    store
        .get(CONSENT_KEY)
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

#[tauri::command]
pub async fn participant_consent_get<R: Runtime>(
    app: AppHandle<R>,
) -> Result<ParticipantConsent, String> {
    Ok(ParticipantConsent {
        enabled: read_consent(&app),
    })
}

#[tauri::command]
pub async fn participant_consent_set<R: Runtime>(
    app: AppHandle<R>,
    enabled: bool,
) -> Result<(), String> {
    let store = app
        .store(CONSENT_STORE)
        .map_err(|e| format!("Failed to open store: {}", e))?;
    store.set(CONSENT_KEY, serde_json::json!(enabled));
    store
        .save()
        .map_err(|e| format!("Failed to save store: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn participant_detect_snapshot<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    meeting_id: String,
) -> Result<DetectionResult, String> {
    // --- 1. Consent gate -----------------------------------------------
    if !read_consent(&app) {
        return Err("AI participant detection requires user consent. Enable it in Settings → Transcription first.".to_string());
    }

    // --- 2. Resolve a vision provider ---------------------------------
    let (endpoint, api_key, model) = resolve_vision_provider(&app, &state).await?;

    // --- 3. Capture the meeting window on a blocking thread -----------
    //
    // xcap is synchronous and platform-specific; keep it off the Tokio
    // runtime so the transcription worker isn't starved.
    let captured = tokio::task::spawn_blocking(window_capture::capture_active_meeting_window)
        .await
        .map_err(|e| format!("Capture thread panicked: {}", e))?
        .map_err(|e| e.to_string())?;

    log::info!(
        "participant_detect_snapshot: captured {}x{} PNG ({} bytes) of {}",
        captured.width,
        captured.height,
        captured.png_bytes.len(),
        captured.source_app
    );

    // --- 4. Call the vision LLM ---------------------------------------
    let provider = vision_client::VisionProvider {
        endpoint,
        api_key,
        model,
    };
    let result = vision_client::detect_participants(
        &provider,
        &captured.png_bytes,
        captured.source_app,
    )
    .await
    .map_err(|e| e.to_string())?;

    // --- 5. Auto-rename a single unnamed cluster if possible ----------
    let pool = state.db_manager.pool();
    let speakers = SpeakersRepository::list_for_meeting(pool, &meeting_id)
        .await
        .map_err(|e| format!("Failed to load speakers: {}", e))?;
    let unnamed: Vec<_> = speakers
        .iter()
        .filter(|s| s.display_name.as_deref().map_or(true, str::is_empty))
        .collect();
    if let (Some(active_name), [only_unnamed]) = (result.current_speaker.as_deref(), unnamed.as_slice()) {
        SpeakersRepository::rename(pool, &only_unnamed.id, Some(active_name))
            .await
            .map_err(|e| format!("Failed to rename speaker: {}", e))?;
        log::info!(
            "Auto-renamed cluster {} -> '{}' from vision snapshot",
            only_unnamed.cluster_idx,
            active_name
        );
    }

    Ok(result)
}

/// Prefer the user's configured custom-OpenAI endpoint when it exists;
/// otherwise fall back to OpenAI with a stored key or `OPENAI_API_KEY`.
async fn resolve_vision_provider<R: Runtime>(
    _app: &AppHandle<R>,
    state: &State<'_, AppState>,
) -> Result<(String, Option<String>, String), String> {
    let pool = state.db_manager.pool();

    // 1. Custom OpenAI (already configured, possibly vision-capable).
    if let Ok(Some(cfg)) = SettingsRepository::get_custom_openai_config(pool).await {
        let endpoint = cfg.endpoint.trim_end_matches('/').to_string();
        if !endpoint.is_empty() {
            let model = if cfg.model.trim().is_empty() {
                "gpt-4o-mini".to_string()
            } else {
                cfg.model
            };
            return Ok((
                format!("{}/chat/completions", endpoint),
                cfg.api_key,
                model,
            ));
        }
    }

    // 2. Stored OpenAI API key or env var fallback.
    let openai_key = SettingsRepository::get_api_key(pool, "openai")
        .await
        .ok()
        .flatten()
        .filter(|k| !k.trim().is_empty())
        .or_else(|| env::var("OPENAI_API_KEY").ok())
        .ok_or_else(|| "No vision-capable provider configured. Add an OpenAI key in Settings or configure a Custom OpenAI endpoint that supports vision.".to_string())?;

    Ok((
        "https://api.openai.com/v1/chat/completions".to_string(),
        Some(openai_key),
        "gpt-4o-mini".to_string(),
    ))
}
