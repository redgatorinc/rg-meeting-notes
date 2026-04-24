//! Per-recording "active session" state for participant detection.
//!
//! Populated at recording-start, cleared at recording-stop. Locks the
//! integrated adapter we're going to use for the entire session so
//! subsequent `participant_detect_snapshot` calls don't wander across
//! adapters mid-meeting.

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::sync::RwLock;
use tauri::{AppHandle, Emitter, Runtime};

use super::adapters::{
    meet_stub::MeetStubAdapter, teams_logs::TeamsLogsAdapter, zoom_logs::ZoomLogsAdapter,
    AdapterStatus, IntegratedAdapter,
};
use super::config;

#[derive(Debug, Clone, Serialize)]
pub struct LockedAdapter {
    pub app_id: String,
    pub app_display_name: String,
    pub method: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParticipantSnapshot {
    pub display_name: String,
    pub source: String, // e.g. "teams/uia"
}

#[derive(Debug, Clone, Serialize)]
pub struct ActiveSession {
    pub meeting_id: Option<String>,
    pub locked_adapter: Option<LockedAdapter>,
    pub detected_at: DateTime<Utc>,
    /// Captured once at recording start from the locked adapter's snapshot.
    /// Persisted to `meeting_participants` by `save_transcript` once the
    /// meeting row exists. Empty if the adapter gave us nothing (e.g. the
    /// "ai" fallback or a Zoom stub).
    #[serde(default)]
    pub participants: Vec<ParticipantSnapshot>,
}

static ACTIVE: RwLock<Option<ActiveSession>> = RwLock::new(None);

pub fn current() -> Option<ActiveSession> {
    ACTIVE.read().ok().and_then(|g| g.clone())
}

pub fn clear() {
    if let Ok(mut g) = ACTIVE.write() {
        *g = None;
    }
}

fn app_display_name(id: &str) -> &'static str {
    match id {
        "teams" => "Microsoft Teams",
        "zoom" => "Zoom",
        "meet" => "Google Meet",
        "ai" => "Vision AI",
        _ => "Unknown",
    }
}

/// Probe adapters, lock the first Ready one. If none are Ready, fall
/// back to an "ai" pseudo-adapter so the screenshot path still has
/// something to tell the UI. Emits `recording-app-detected` either way.
pub fn detect_and_lock<R: Runtime>(app: &AppHandle<R>) -> ActiveSession {
    let cfg = config::load(app);

    let adapters: Vec<Box<dyn IntegratedAdapter>> = vec![
        Box::new(super::adapters::teams_uia::TeamsUiaAdapter::new()),
        Box::new(TeamsLogsAdapter::new()),
        Box::new(ZoomLogsAdapter::new()),
        Box::new(MeetStubAdapter::new()),
    ];

    let mut locked: Option<LockedAdapter> = None;
    let mut participants: Vec<ParticipantSnapshot> = Vec::new();

    if cfg.integrated.enabled {
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
            if matches!(adapter.status(), AdapterStatus::Ready) {
                // Capture the participant list at lock time. Best-effort;
                // adapter.snapshot() can fail (UIA tree race, empty roster)
                // and that's fine — cue parser + LLM passes still run.
                let source = format!("{}/{}", adapter.id(), "lock");
                if let Ok(snap) = adapter.snapshot() {
                    participants.extend(snap.participants.into_iter().map(|p| {
                        ParticipantSnapshot {
                            display_name: p,
                            source: source.clone(),
                        }
                    }));
                }
                locked = Some(LockedAdapter {
                    app_id: adapter.id().to_string(),
                    app_display_name: app_display_name(adapter.id()).to_string(),
                    method: "log_tail".to_string(),
                });
                break;
            }
        }
    }

    if locked.is_none() {
        // Fall back to a pseudo "ai" lock; LiveParticipantStatus still
        // works because the AI path uses xcap + vision directly.
        let app_name = crate::meeting_detector::active_meeting_app()
            .map(|m| m.display_name.to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        locked = Some(LockedAdapter {
            app_id: "ai".to_string(),
            app_display_name: app_name,
            method: "vision".to_string(),
        });
    }

    let session = ActiveSession {
        meeting_id: None,
        locked_adapter: locked,
        detected_at: Utc::now(),
        participants,
    };

    if let Ok(mut g) = ACTIVE.write() {
        *g = Some(session.clone());
    }
    let _ = app.emit("recording-app-detected", &session);
    log::info!(
        "participant_detection: locked adapter for recording session: {:?}",
        session.locked_adapter
    );
    session
}
