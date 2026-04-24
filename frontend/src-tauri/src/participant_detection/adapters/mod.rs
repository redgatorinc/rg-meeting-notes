//! Per-app "Integrated" adapters for participant detection.
//!
//! Each adapter tries to read the meeting roster + current speaker from
//! an app-specific side channel (log files, accessibility trees, or a
//! companion browser extension) WITHOUT using AI and without joining
//! the meeting. Fragile by design — a Teams/Zoom point release may
//! break the format we parse. That's why the UI exposes this as a
//! "Beta" mode and the overall detection pipeline defaults to
//! `IntegratedWithAiFallback` so any adapter failure is transparent.

pub mod meet_stub;
pub mod teams_logs;
pub mod zoom_logs;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AdapterStatus {
    /// Adapter detected the app is running and believes it can produce a snapshot.
    Ready,
    /// The target app is not running right now.
    NotDetected,
    /// The adapter cannot support this platform / app version
    /// (e.g. Meet log-tail: there is no such log).
    Unsupported { reason: String },
    /// A transient error — e.g. permissions denied, log file locked.
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterSnapshot {
    pub participants: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_speaker: Option<String>,
    /// Stable identifier for telemetry + UI badging, e.g.
    /// `"teams/log_tail"` or `"zoom/a11y"`.
    pub source: String,
}

pub trait IntegratedAdapter: Send + Sync {
    /// Short identifier: `"teams"`, `"zoom"`, `"meet"`.
    fn id(&self) -> &'static str;

    /// Current readiness; polled by the UI to render the status badge.
    fn status(&self) -> AdapterStatus;

    /// Best-effort snapshot; returns `Err` if the adapter is not ready
    /// or failed this call.
    fn snapshot(&self) -> anyhow::Result<AdapterSnapshot>;
}
