//! AI participant detection.
//!
//! Capture *only* the active meeting app's window (never the desktop, never
//! other apps) and pass the PNG to a vision-capable LLM that returns the
//! visible participant names + an optional current-speaker hint. The result
//! is used to auto-rename anonymous `speakers.cluster_idx` rows produced by
//! the diarizer.
//!
//! Phase 1 (this module, MVP): manual button, cloud provider only
//! (`gpt-4o-mini` via OpenAI / custom-openai / any OpenAI-compatible vision
//! endpoint), one-time consent modal. Auto-trigger on `speaker-joined`
//! and the local Moondream2 path land in follow-up PRs. See the plan file.

pub mod adapters;
pub mod commands;
pub mod config;
pub mod vision_client;
pub mod window_capture;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Participant {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionResult {
    pub participants: Vec<Participant>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_speaker: Option<String>,
    pub confidence: f32,
    /// Host component of the URL the image was POSTed to, so the frontend
    /// can show it in the "sending to …" toast ("api.openai.com" etc.).
    pub provider_host: String,
    /// Meeting app whose window we captured (e.g. "Microsoft Teams").
    pub source_app: String,
}
