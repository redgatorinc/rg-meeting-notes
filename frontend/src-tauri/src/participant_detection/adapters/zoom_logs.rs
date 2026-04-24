//! Zoom — log-file-tail adapter (Phase-A placeholder).
//!
//! Same rationale as `teams_logs.rs`: report `Ready` when the Zoom log
//! directory exists so the UI shows "Ready" in Settings, but return
//! `Err` from `snapshot()` until the robust parser lands. That lets
//! `IntegratedWithAiFallback` transparently pick the AI path.

use anyhow::{anyhow, Result};
use std::path::PathBuf;

use super::{AdapterSnapshot, AdapterStatus, IntegratedAdapter};

pub struct ZoomLogsAdapter;

impl ZoomLogsAdapter {
    pub fn new() -> Self {
        Self
    }
}

fn log_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        return dirs::data_dir().map(|p| p.join("Zoom/logs"));
    }
    #[cfg(target_os = "macos")]
    {
        return dirs::home_dir()
            .map(|p| p.join("Library/Application Support/zoom.us/logs"));
    }
    #[cfg(target_os = "linux")]
    {
        return dirs::home_dir().map(|p| p.join(".zoom/logs"));
    }
    #[allow(unreachable_code)]
    None
}

impl IntegratedAdapter for ZoomLogsAdapter {
    fn id(&self) -> &'static str {
        "zoom"
    }

    fn status(&self) -> AdapterStatus {
        match log_dir() {
            Some(dir) if dir.exists() => AdapterStatus::Ready,
            _ => AdapterStatus::NotDetected,
        }
    }

    fn snapshot(&self) -> Result<AdapterSnapshot> {
        // TODO(phase-A+): tail zoom_stdout_stderr.log, parse
        // "Got active speaker" / "User <name> joined" entries.
        Err(anyhow!(
            "zoom/log_tail: parser not yet implemented — falling back to AI path"
        ))
    }
}
