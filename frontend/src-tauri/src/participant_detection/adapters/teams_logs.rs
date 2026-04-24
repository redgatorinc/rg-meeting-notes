//! Microsoft Teams — log-file-tail adapter.
//!
//! Phase-A placeholder: reports `Ready` when the Teams log directory is
//! present and contains a recent file, but does **not** yet parse the
//! logs. The log format varies between Teams classic and "new Teams"
//! (the MSTeams_* WinStore package), and we need live samples to write
//! robust regexes. Returning `NotDetected` from `snapshot()` lets the
//! IntegratedWithAiFallback flow fall through to the AI path cleanly
//! until the parser lands in a follow-up.

use anyhow::{anyhow, Result};
use std::path::PathBuf;

use super::{AdapterSnapshot, AdapterStatus, IntegratedAdapter};

pub struct TeamsLogsAdapter;

impl TeamsLogsAdapter {
    pub fn new() -> Self {
        Self
    }
}

fn candidate_log_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    #[cfg(target_os = "windows")]
    {
        if let Some(local) = dirs::data_local_dir() {
            // New Teams (MSTeams_*): %LOCALAPPDATA%\Packages\MSTeams_*\LocalCache\Microsoft\MSTeams\Logs
            if let Ok(read) = std::fs::read_dir(local.join("Packages")) {
                for entry in read.flatten() {
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    if name.starts_with("MSTeams_") {
                        let p = entry
                            .path()
                            .join("LocalCache/Microsoft/MSTeams/Logs");
                        if p.exists() {
                            dirs.push(p);
                        }
                    }
                }
            }
        }
        if let Some(roaming) = dirs::data_dir() {
            // Teams classic
            dirs.push(roaming.join("Microsoft/Teams"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            dirs.push(home.join("Library/Application Support/Microsoft/Teams"));
            dirs.push(home.join("Library/Containers/com.microsoft.teams2/Data/Library/Application Support/com.microsoft.teams2"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(config) = dirs::config_dir() {
            dirs.push(config.join("Microsoft/Microsoft Teams"));
        }
    }

    dirs
}

impl IntegratedAdapter for TeamsLogsAdapter {
    fn id(&self) -> &'static str {
        "teams"
    }

    fn status(&self) -> AdapterStatus {
        for dir in candidate_log_dirs() {
            if dir.exists() {
                return AdapterStatus::Ready;
            }
        }
        AdapterStatus::NotDetected
    }

    fn snapshot(&self) -> Result<AdapterSnapshot> {
        // TODO(phase-A+): parse RosterUpdate / ActiveSpeakerChanged lines
        // from the latest log file in the candidate dirs. Until we have
        // live log samples the robust thing is to return an error so
        // IntegratedWithAiFallback uses the vision path.
        Err(anyhow!(
            "teams/log_tail: Teams log parser is not yet implemented (the Integrated Beta currently only reports that the app is running). Switch detection mode to 'Integrated + AI fallback' to use screenshot + vision instead."
        ))
    }
}
