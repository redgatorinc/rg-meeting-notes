//! Microsoft Teams — log-file-tail adapter.
//!
//! Best-effort parser. Teams (especially new Teams / MSTeams_*) writes
//! a rotating set of `.log` files with a mix of JSON event blobs and
//! plain-text diagnostic lines. We:
//!   1. Locate the most recent `.log` file in the candidate directories
//!      below (new Teams, Teams classic, macOS variants).
//!   2. Read the tail of that file (we cap at 512 KB so tailing never
//!      blocks the detect call on a multi-GB log).
//!   3. Scan the tail with two regex sets: one for participant display
//!      names, one for the current "active speaker" hint. Teams format
//!      changes between releases; when these stop finding anything we
//!      will iterate. The adapter fails cleanly (Err) when the tail is
//!      empty / unparseable, so `Integrated + AI fallback` mode still
//!      produces a result via the vision path.
//!
//! Never opens logs outside Teams' known dirs, never transmits log
//! contents off the device, and never parses older-than-current logs.

use anyhow::{anyhow, Context, Result};
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

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
                        let p2 = entry
                            .path()
                            .join("LocalCache/Microsoft/MSTeams/EBWebView/logs");
                        if p2.exists() {
                            dirs.push(p2);
                        }
                    }
                }
            }
        }
        if let Some(roaming) = dirs::data_dir() {
            dirs.push(roaming.join("Microsoft/Teams"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            dirs.push(home.join("Library/Application Support/Microsoft/Teams"));
            dirs.push(home.join("Library/Containers/com.microsoft.teams2/Data/Library/Application Support/com.microsoft.teams2"));
            dirs.push(home.join("Library/Logs/Microsoft/MSTeams"));
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

fn newest_log_file(dirs: &[PathBuf]) -> Option<PathBuf> {
    let mut best: Option<(PathBuf, SystemTime)> = None;
    for dir in dirs {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if !(name.ends_with(".log") || name.ends_with(".txt")) {
                continue;
            }
            let modified = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            if best
                .as_ref()
                .map_or(true, |(_, ts)| modified > *ts)
            {
                best = Some((path, modified));
            }
        }
    }
    best.map(|(p, _)| p)
}

/// Read at most `max_bytes` from the end of the file.
fn tail(path: &Path, max_bytes: u64) -> Result<String> {
    let mut f = fs::File::open(path).context("open log")?;
    let len = f.metadata()?.len();
    let start = len.saturating_sub(max_bytes);
    f.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::with_capacity(max_bytes.min(len) as usize);
    f.read_to_end(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Lift plausible participant display names out of the log tail.
fn extract_participants(tail: &str) -> HashSet<String> {
    let mut names = HashSet::new();

    // JSON `"displayName": "Alice Johnson"` — the most reliable.
    let re_display = Regex::new(r#""displayName"\s*:\s*"([^"<>]{2,80})""#).unwrap();
    for cap in re_display.captures_iter(tail) {
        push_name(&mut names, &cap[1]);
    }

    // `"name": "Alice Johnson"` tight constraint so we don't scoop log filenames.
    let re_name = Regex::new(r#""name"\s*:\s*"([A-Z][a-zA-Z'\-]+(?:\s+[A-Z][a-zA-Z'\-]+){1,3})""#).unwrap();
    for cap in re_name.captures_iter(tail) {
        push_name(&mut names, &cap[1]);
    }

    // `"upn": "alice@..."` — fall back to the local part when no display name was seen.
    let re_upn = Regex::new(r#""upn"\s*:\s*"([a-zA-Z0-9._\-]+)@"#).unwrap();
    for cap in re_upn.captures_iter(tail) {
        // Only use UPNs when we don't already have anyone with that prefix.
        let local = cap[1].replace('.', " ");
        push_name(&mut names, &title_case(&local));
    }

    names
}

/// Best guess for the current speaker: the most-recent `activeSpeaker` /
/// dominant-speaker mention in the tail.
fn extract_current_speaker(tail: &str) -> Option<String> {
    let patterns = [
        Regex::new(r#"(?i)active[_\s-]?speaker["']?\s*[:=]\s*["']?([A-Z][a-zA-Z'\-]+(?:\s+[A-Z][a-zA-Z'\-]+){0,3})"#).unwrap(),
        Regex::new(r#"(?i)dominant[_\s-]?speaker["']?\s*[:=]\s*["']?([A-Z][a-zA-Z'\-]+(?:\s+[A-Z][a-zA-Z'\-]+){0,3})"#).unwrap(),
    ];
    let mut latest: Option<(usize, String)> = None;
    for re in &patterns {
        for m in re.captures_iter(tail) {
            let pos = m.get(0).map(|x| x.start()).unwrap_or(0);
            let name = m.get(1).map(|x| x.as_str().trim().to_string()).unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            if latest.as_ref().map_or(true, |(p, _)| pos > *p) {
                latest = Some((pos, name));
            }
        }
    }
    latest.map(|(_, n)| n)
}

fn push_name(set: &mut HashSet<String>, raw: &str) {
    let trimmed = raw.trim();
    // Cheap filters to avoid adding garbage like log codes or file paths.
    if trimmed.len() < 2 || trimmed.len() > 80 {
        return;
    }
    if trimmed.chars().any(|c| c == '/' || c == '\\' || c == '\0') {
        return;
    }
    if trimmed.chars().all(|c| c.is_ascii_digit()) {
        return;
    }
    set.insert(trimmed.to_string());
}

fn title_case(s: &str) -> String {
    s.split(' ')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            let first = chars.next().unwrap().to_ascii_uppercase();
            let rest: String = chars.collect();
            format!("{}{}", first, rest)
        })
        .collect::<Vec<_>>()
        .join(" ")
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
        let dirs = candidate_log_dirs();
        if dirs.is_empty() {
            return Err(anyhow!("No Teams log directory found."));
        }
        let newest = newest_log_file(&dirs)
            .ok_or_else(|| anyhow!("No .log file in Teams log directories."))?;

        let text = tail(&newest, 512 * 1024)
            .with_context(|| format!("Tailing {}", newest.display()))?;

        let names = extract_participants(&text);
        if names.is_empty() {
            return Err(anyhow!(
                "teams/log_tail: no participant names found in the last 512 KB of {}. The log format may have changed. Switch to 'Integrated + AI fallback' for a screenshot-based result.",
                newest.file_name().and_then(|s| s.to_str()).unwrap_or("(latest log)")
            ));
        }

        let current = extract_current_speaker(&text);
        let mut participants: Vec<String> = names.into_iter().collect();
        participants.sort();

        Ok(AdapterSnapshot {
            participants,
            current_speaker: current,
            source: "teams/log_tail".to_string(),
        })
    }
}
