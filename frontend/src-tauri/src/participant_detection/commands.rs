//! Tauri commands for AI participant detection.
//!
//! Entry points:
//!   - `participant_config_get` / `participant_config_set` — full JSON
//!     config (see `config.rs`). Replaces PR #8's simple consent bool.
//!   - `participant_adapter_statuses` — for the Settings panel badges.
//!   - `participant_detect_snapshot` — run detection now using the
//!     configured mode. Routing:
//!         integrated  → try each enabled adapter, first Ok wins
//!         ai          → xcap + vision (PR #8 path)
//!         hybrid      → integrated first; on failure fall back to ai
//!
//! Back-compat: the old `participant_consent_get/set` commands are
//! kept as thin wrappers so the frontend (not yet migrated in this PR)
//! keeps working.

use std::env;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime, State};

use super::adapters::{
    meet_stub::MeetStubAdapter, teams_logs::TeamsLogsAdapter, zoom_logs::ZoomLogsAdapter,
    AdapterStatus, IntegratedAdapter,
};
use super::config::{self, AdapterMethod, AiSource, DetectionMode, ParticipantDetectionConfig};
use super::{vision_client, window_capture, DetectionResult, Participant};
use crate::database::repositories::setting::SettingsRepository;
use crate::database::repositories::speaker::SpeakersRepository;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Config commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn participant_config_get<R: Runtime>(
    app: AppHandle<R>,
) -> Result<ParticipantDetectionConfig, String> {
    Ok(config::load(&app))
}

#[tauri::command]
pub async fn participant_config_set<R: Runtime>(
    app: AppHandle<R>,
    config: ParticipantDetectionConfig,
) -> Result<(), String> {
    self::config::save(&app, &config).map_err(|e| e.to_string())
}

// Back-compat shims for the PR #8 frontend call sites. These forward
// to / read from the `enabled` field of the new config so we don't
// break the SpeakersPanel UI until the Settings section replaces it.

#[derive(Debug, Serialize, Deserialize)]
pub struct ParticipantConsent {
    pub enabled: bool,
}

#[tauri::command]
pub async fn participant_consent_get<R: Runtime>(
    app: AppHandle<R>,
) -> Result<ParticipantConsent, String> {
    let cfg = config::load(&app);
    Ok(ParticipantConsent {
        enabled: cfg.enabled,
    })
}

#[tauri::command]
pub async fn participant_consent_set<R: Runtime>(
    app: AppHandle<R>,
    enabled: bool,
) -> Result<(), String> {
    let mut cfg = config::load(&app);
    cfg.enabled = enabled;
    if enabled && cfg.consent_accepted_at.is_none() {
        cfg.consent_accepted_at = Some(chrono::Utc::now().to_rfc3339());
    }
    config::save(&app, &cfg).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Adapter status (drives Settings badges)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct AdapterStatusReport {
    pub id: &'static str,
    pub status: AdapterStatus,
}

#[tauri::command]
pub async fn participant_adapter_statuses() -> Result<Vec<AdapterStatusReport>, String> {
    let adapters = build_adapters();
    Ok(adapters
        .iter()
        .map(|a| AdapterStatusReport {
            id: a.id(),
            status: a.status(),
        })
        .collect())
}

fn build_adapters() -> Vec<Box<dyn IntegratedAdapter>> {
    vec![
        Box::new(TeamsLogsAdapter::new()),
        Box::new(ZoomLogsAdapter::new()),
        Box::new(MeetStubAdapter::new()),
    ]
}

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/// Run detection now. `meeting_id` is optional — when provided, the
/// backend will auto-rename a single unnamed speaker cluster in that
/// meeting from `current_speaker`. When `None` (live from the Home
/// screen before any meeting row is finalised) the result is returned
/// without touching the DB so the floating status card can just show it.
#[tauri::command]
pub async fn participant_detect_snapshot<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    meeting_id: Option<String>,
) -> Result<DetectionResult, String> {
    let cfg = config::load(&app);
    if !cfg.enabled {
        return Err("Participant detection is disabled. Enable it in Settings → Transcription → Participants Detection.".to_string());
    }

    let mid = meeting_id.as_deref();
    match cfg.mode {
        DetectionMode::Integrated => run_integrated(&cfg, &state, mid).await,
        DetectionMode::Ai => run_ai(&app, &state, mid, &cfg).await,
        DetectionMode::IntegratedWithAiFallback => {
            match run_integrated(&cfg, &state, mid).await {
                Ok(result) => Ok(result),
                Err(err) => {
                    log::info!(
                        "Integrated detection failed ({}); falling back to AI path",
                        err
                    );
                    run_ai(&app, &state, mid, &cfg).await
                }
            }
        }
    }
}

async fn run_integrated(
    cfg: &ParticipantDetectionConfig,
    state: &State<'_, AppState>,
    meeting_id: Option<&str>,
) -> Result<DetectionResult, String> {
    if !cfg.integrated.enabled {
        return Err("Integrated detection is disabled in settings.".to_string());
    }

    let adapters = build_adapters();
    let mut last_err: Option<String> = None;

    for adapter in adapters.iter() {
        let enabled = match adapter.id() {
            "teams" => cfg.integrated.teams.enabled,
            "zoom" => cfg.integrated.zoom.enabled,
            "meet" => cfg.integrated.meet.enabled,
            _ => false,
        };
        if !enabled {
            continue;
        }
        match adapter.snapshot() {
            Ok(snapshot) => {
                let result = DetectionResult {
                    participants: snapshot
                        .participants
                        .into_iter()
                        .map(|name| Participant { name })
                        .collect(),
                    current_speaker: snapshot.current_speaker,
                    confidence: 1.0,
                    provider_host: snapshot.source,
                    source_app: adapter.id().to_string(),
                };
                if let Some(mid) = meeting_id {
                    auto_rename_if_single(state, mid, &result).await?;
                }
                return Ok(result);
            }
            Err(e) => last_err = Some(format!("{}: {}", adapter.id(), e)),
        }
    }

    Err(last_err.unwrap_or_else(|| "No integrated adapters are enabled.".to_string()))
}

async fn run_ai<R: Runtime>(
    _app: &AppHandle<R>,
    state: &State<'_, AppState>,
    meeting_id: Option<&str>,
    cfg: &ParticipantDetectionConfig,
) -> Result<DetectionResult, String> {
    if matches!(cfg.ai.source, AiSource::Local) {
        return Err(
            "Local vision model is not yet installed. Open Settings → Participants Detection → Manage downloads to install Moondream2."
                .to_string(),
        );
    }

    let (endpoint, api_key, model) = resolve_external_provider(state, cfg).await?;

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

    let provider = vision_client::VisionProvider {
        endpoint,
        api_key,
        model,
    };
    let result =
        vision_client::detect_participants(&provider, &captured.png_bytes, captured.source_app)
            .await
            .map_err(|e| e.to_string())?;

    if let Some(mid) = meeting_id {
        auto_rename_if_single(state, mid, &result).await?;
    }
    Ok(result)
}

async fn auto_rename_if_single(
    state: &State<'_, AppState>,
    meeting_id: &str,
    result: &DetectionResult,
) -> Result<(), String> {
    let pool = state.db_manager.pool();
    let speakers = SpeakersRepository::list_for_meeting(pool, meeting_id)
        .await
        .map_err(|e| format!("Failed to load speakers: {}", e))?;
    let unnamed: Vec<_> = speakers
        .iter()
        .filter(|s| s.display_name.as_deref().map_or(true, str::is_empty))
        .collect();
    if let (Some(active_name), [only_unnamed]) =
        (result.current_speaker.as_deref(), unnamed.as_slice())
    {
        SpeakersRepository::rename(pool, &only_unnamed.id, Some(active_name))
            .await
            .map_err(|e| format!("Failed to rename speaker: {}", e))?;
        log::info!(
            "Auto-renamed cluster {} -> '{}' from detection result",
            only_unnamed.cluster_idx,
            active_name
        );
    }
    Ok(())
}

async fn resolve_external_provider(
    state: &State<'_, AppState>,
    cfg: &ParticipantDetectionConfig,
) -> Result<(String, Option<String>, String), String> {
    let pool = state.db_manager.pool();

    if cfg.ai.external.same_as_summary {
        // Reuse whatever the user has configured in Settings → Summary.
        if let Ok(Some(model_cfg)) = SettingsRepository::get_model_config(pool).await {
            // Custom OpenAI gets special treatment — it lives in a different row.
            if model_cfg.provider == "custom-openai" {
                if let Ok(Some(custom)) = SettingsRepository::get_custom_openai_config(pool).await {
                    let endpoint = custom.endpoint.trim_end_matches('/').to_string();
                    if !endpoint.is_empty() {
                        return Ok((
                            format!("{}/chat/completions", endpoint),
                            custom.api_key,
                            if custom.model.trim().is_empty() {
                                "gpt-4o-mini".into()
                            } else {
                                custom.model
                            },
                        ));
                    }
                }
            }
            // Otherwise fall through to provider-specific endpoint.
            if model_cfg.provider == "openai" {
                let key = SettingsRepository::get_api_key(pool, "openai")
                    .await
                    .ok()
                    .flatten()
                    .or_else(|| env::var("OPENAI_API_KEY").ok())
                    .ok_or_else(|| "Summary model is OpenAI but no API key is configured.".to_string())?;
                return Ok((
                    "https://api.openai.com/v1/chat/completions".to_string(),
                    Some(key),
                    if model_cfg.model.is_empty() {
                        "gpt-4o-mini".into()
                    } else {
                        model_cfg.model
                    },
                ));
            }
        }
    }

    // Explicit external config (not "same as summary", or summary lookup failed).
    let provider_id = cfg
        .ai
        .external
        .provider
        .as_deref()
        .unwrap_or("openai");
    let model = cfg
        .ai
        .external
        .model
        .clone()
        .unwrap_or_else(|| "gpt-4o-mini".into());

    match provider_id {
        "custom-openai" => {
            let custom = SettingsRepository::get_custom_openai_config(pool)
                .await
                .ok()
                .flatten()
                .ok_or_else(|| "Custom OpenAI config not found.".to_string())?;
            let endpoint = custom.endpoint.trim_end_matches('/').to_string();
            if endpoint.is_empty() {
                return Err("Custom OpenAI endpoint is empty.".to_string());
            }
            Ok((
                format!("{}/chat/completions", endpoint),
                custom.api_key,
                if custom.model.trim().is_empty() {
                    model
                } else {
                    custom.model
                },
            ))
        }
        _ => {
            let key = SettingsRepository::get_api_key(pool, "openai")
                .await
                .ok()
                .flatten()
                .or_else(|| env::var("OPENAI_API_KEY").ok())
                .ok_or_else(|| "No OpenAI API key configured.".to_string())?;
            Ok((
                "https://api.openai.com/v1/chat/completions".to_string(),
                Some(key),
                model,
            ))
        }
    }
}
