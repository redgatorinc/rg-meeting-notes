use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};
use sysinfo::System;
use tauri::{AppHandle, Emitter, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};
use xcap::Window;

/// Meeting detection rules.
///
/// Each entry has:
/// - `display_name`: shown in the UI banner
/// - `app_processes`: the main app process (must be running as a precondition)
/// - `meeting_indicators`: processes that only appear during an active meeting/call.
///   If empty, the app process itself is treated as the indicator (for apps
///   where the process only launches when joining a call).
/// Static description of a supported meeting app. Visible to external
/// modules (e.g. `participant_detection::window_capture`) which need the
/// list of possible window-owning process names to filter the enumerated
/// OS windows down to the meeting one.
pub struct MeetingApp {
    pub display_name: &'static str,
    pub app_processes: &'static [&'static str],
    pub meeting_indicators: &'static [&'static str],
}

const MEETING_APPS: &[MeetingApp] = &[
    MeetingApp {
        display_name: "Zoom",
        app_processes: &["zoom.us"],
        meeting_indicators: &["cpthost"],
    },
    MeetingApp {
        display_name: "Feishu",
        app_processes: &["feishu", "lark"],
        meeting_indicators: &["feishu_vc", "lark_vc", "byteaudiod"],
    },
    MeetingApp {
        display_name: "Tencent Meeting",
        app_processes: &["wemeet"],
        meeting_indicators: &["wemeetapp"],
    },
    MeetingApp {
        display_name: "VooV Meeting",
        app_processes: &["voov"],
        meeting_indicators: &[],
    },
    MeetingApp {
        display_name: "Microsoft Teams",
        app_processes: &["microsoft teams", "ms-teams", "teams"],
        meeting_indicators: &[],
    },
    MeetingApp {
        display_name: "Discord",
        app_processes: &["discord"],
        meeting_indicators: &[],
    },
    MeetingApp {
        display_name: "Webex",
        app_processes: &["webex", "webexmta"],
        meeting_indicators: &["ciscocollabhost"],
    },
];

const BROWSER_PROCESS_PATTERNS: &[&str] = &[
    "arc",
    "brave",
    "chrome",
    "chromium",
    "firefox",
    "google chrome",
    "microsoft edge",
    "msedge",
    "safari",
];

const TEAMS_MEETING_WINDOW_PATTERNS: &[&str] = &[
    "meeting",
    "call",
    "calling",
    "screen sharing",
    "share tray",
    "meeting controls",
    "live event",
    "town hall",
];

const TEAMS_MEETING_LOG_PATTERNS: &[&str] = &[
    "activespeaker",
    "active speaker",
    "dominantspeaker",
    "dominant speaker",
    "meetingstage",
    "meeting stage",
    "call state",
    "callstate",
    "joined meeting",
    "meeting joined",
];

static GOOGLE_MEET_APP: MeetingApp = MeetingApp {
    display_name: "Google Meet",
    app_processes: BROWSER_PROCESS_PATTERNS,
    meeting_indicators: &[],
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingAppDetected {
    pub app_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingDetectionSnapshot {
    pub enabled: bool,
    pub active_apps: Vec<String>,
}

pub struct MeetingDetectionState {
    enabled: AtomicBool,
}

impl MeetingDetectionState {
    pub fn new() -> Self {
        Self {
            enabled: AtomicBool::new(true),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }
}

fn has_process(system: &System, patterns: &[&str]) -> bool {
    for process in system.processes().values() {
        let name = process.name().to_string_lossy().to_lowercase();
        for &p in patterns {
            if name.contains(p) {
                return true;
            }
        }
    }
    false
}

fn scan_active_meetings(system: &mut System) -> HashSet<String> {
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let mut active: HashSet<String> = HashSet::new();
    for app in MEETING_APPS {
        if !has_process(system, app.app_processes) {
            continue;
        }
        if app.display_name == "Microsoft Teams" {
            if has_teams_meeting_signal() {
                active.insert(app.display_name.to_string());
            }
            continue;
        }
        if app.meeting_indicators.is_empty() {
            if has_active_meeting_window(app, &["meeting", "call"]) {
                active.insert(app.display_name.to_string());
            }
        } else if has_process(system, app.meeting_indicators) {
            active.insert(app.display_name.to_string());
        }
    }
    if has_google_meet_window() {
        active.insert("Google Meet".to_string());
    }
    active
}

fn has_teams_meeting_signal() -> bool {
    has_active_meeting_window_by_patterns(
        &["microsoft teams", "ms-teams", "teams"],
        TEAMS_MEETING_WINDOW_PATTERNS,
    ) || has_recent_teams_meeting_log_activity()
}

fn has_active_meeting_window(app: &MeetingApp, title_patterns: &[&str]) -> bool {
    has_active_meeting_window_by_patterns(app.app_processes, title_patterns)
}

fn has_active_meeting_window_by_patterns(owner_patterns: &[&str], title_patterns: &[&str]) -> bool {
    let Ok(windows) = Window::all() else {
        return false;
    };

    windows.into_iter().any(|win| {
        let owner = win.app_name().unwrap_or_default().to_lowercase();
        let title = win.title().unwrap_or_default().to_lowercase();
        if title.is_empty() {
            return false;
        }
        let owner_matches = owner_patterns.iter().any(|pattern| {
            let pattern = pattern.to_lowercase();
            owner.contains(&pattern) || title.contains(&pattern)
        });
        owner_matches
            && title_patterns
                .iter()
                .any(|pattern| title.contains(&pattern.to_lowercase()))
    })
}

fn has_google_meet_window() -> bool {
    let Ok(windows) = Window::all() else {
        return false;
    };

    windows.into_iter().any(|win| {
        let owner = win.app_name().unwrap_or_default().to_lowercase();
        let title = win.title().unwrap_or_default().to_lowercase();
        let is_browser = BROWSER_PROCESS_PATTERNS
            .iter()
            .any(|pattern| owner.contains(pattern));
        is_browser
            && (title.contains("google meet")
                || title.contains("meet.google.com")
                || title.starts_with("meet - "))
    })
}

fn has_recent_teams_meeting_log_activity() -> bool {
    for path in recent_teams_log_files(8) {
        let Ok(modified) = fs::metadata(&path).and_then(|m| m.modified()) else {
            continue;
        };
        let Ok(age) = SystemTime::now().duration_since(modified) else {
            continue;
        };
        if age > Duration::from_secs(120) {
            continue;
        }
        let Ok(text) = tail_file(&path, 256 * 1024) else {
            continue;
        };
        let text = text.to_lowercase();
        if TEAMS_MEETING_LOG_PATTERNS
            .iter()
            .any(|pattern| text.contains(pattern))
        {
            return true;
        }
    }
    false
}

fn recent_teams_log_files(max_files: usize) -> Vec<PathBuf> {
    let mut files: Vec<(PathBuf, SystemTime)> = Vec::new();
    for dir in teams_log_dirs() {
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
            files.push((path, modified));
        }
    }
    files.sort_by(|a, b| b.1.cmp(&a.1));
    files
        .into_iter()
        .take(max_files)
        .map(|(path, _)| path)
        .collect()
}

fn teams_log_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    #[cfg(target_os = "windows")]
    {
        if let Some(local) = dirs::data_local_dir() {
            if let Ok(read) = fs::read_dir(local.join("Packages")) {
                for entry in read.flatten() {
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    if name.starts_with("MSTeams_") {
                        dirs.push(entry.path().join("LocalCache/Microsoft/MSTeams/Logs"));
                        dirs.push(entry.path().join("LocalCache/Microsoft/MSTeams/EBWebView/logs"));
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

    dirs.into_iter().filter(|path| path.exists()).collect()
}

fn tail_file(path: &Path, max_bytes: u64) -> std::io::Result<String> {
    let mut file = fs::File::open(path)?;
    let len = file.metadata()?.len();
    let start = len.saturating_sub(max_bytes);
    file.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::with_capacity(max_bytes.min(len) as usize);
    file.read_to_end(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn active_meeting_names(system: &mut System) -> Vec<String> {
    let mut active_apps: Vec<String> = scan_active_meetings(system).into_iter().collect();
    active_apps.sort();
    active_apps
}

const BANNER_WINDOW_LABEL: &str = "meeting-banner";
const BANNER_WIDTH: f64 = 420.0;
const BANNER_HEIGHT: f64 = 64.0;

/// Show the floating banner window for a detected meeting app.
fn show_banner_window<R: Runtime>(app_handle: &AppHandle<R>, app_name: &str) {
    // If the banner window already exists, just update & show it
    if let Some(win) = app_handle.get_webview_window(BANNER_WINDOW_LABEL) {
        let _ = win.emit("meeting-app-detected", MeetingAppDetected {
            app_name: app_name.to_string(),
        });
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }

    // Build the URL with the app name as a query parameter
    let url_str = format!("/meeting-banner?app={}", urlencoded(app_name));
    let url = WebviewUrl::App(url_str.into());

    // Get primary monitor to center the window horizontally at top
    let x = app_handle
        .primary_monitor()
        .ok()
        .flatten()
        .map(|m| {
            let size = m.size();
            ((size.width as f64 / m.scale_factor()) - BANNER_WIDTH) / 2.0
        })
        .unwrap_or(500.0);

    match WebviewWindowBuilder::new(app_handle, BANNER_WINDOW_LABEL, url)
        .title("Start AI Notes")
        .inner_size(BANNER_WIDTH, BANNER_HEIGHT)
        .position(x, 36.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .build()
    {
        Ok(_) => info!("Banner window created for: {}", app_name),
        Err(e) => warn!("Failed to create banner window: {}", e),
    }
}

/// Simple percent-encoding for the app name in query string.
fn urlencoded(s: &str) -> String {
    s.replace(' ', "%20")
}

pub fn start_detection_loop<R: Runtime>(app_handle: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        let mut system = System::new();

        let mut known_meetings = scan_active_meetings(&mut system);
        if !known_meetings.is_empty() {
            info!(
                "Meetings already active at startup (will not notify): {:?}",
                known_meetings
            );
        }

        let mut notified: HashSet<String> = HashSet::new();

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;

            let state: tauri::State<'_, MeetingDetectionState> = match app_handle.try_state() {
                Some(s) => s,
                None => continue,
            };
            if !state.is_enabled() {
                continue;
            }

            if crate::audio::recording_commands::is_recording().await {
                continue;
            }

            let currently_active = scan_active_meetings(&mut system);
            let mut active_apps: Vec<String> = currently_active.iter().cloned().collect();
            active_apps.sort();
            let _ = app_handle.emit("meeting-detection-updated", MeetingDetectionSnapshot {
                enabled: true,
                active_apps,
            });

            for app in &currently_active {
                if !known_meetings.contains(app) && !notified.contains(app) {
                    info!("Meeting started in: {}", app);
                    notified.insert(app.clone());
                    show_banner_window(&app_handle, app);
                }
            }

            known_meetings.retain(|a| currently_active.contains(a));
            notified.retain(|a| currently_active.contains(a));

            known_meetings = currently_active;
        }
    });
}

/// Close the banner popup window (called from the banner UI).
#[tauri::command]
pub async fn dismiss_meeting_banner<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(BANNER_WINDOW_LABEL) {
        win.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Close banner and bring main window to front to start recording.
#[tauri::command]
pub async fn accept_meeting_banner<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    // Close banner
    if let Some(win) = app.get_webview_window(BANNER_WINDOW_LABEL) {
        let _ = win.close();
    }

    // Focus main window and trigger recording start
    if let Some(main_win) = app.get_webview_window("main") {
        let _ = main_win.unminimize();
        let _ = main_win.show();
        let _ = main_win.set_focus();
        // Set the auto-start flag and navigate to home
        let _ = main_win.eval("sessionStorage.setItem('autoStartRecording', 'true')");
        let _ = main_win.eval("window.location.assign('/')");
    }
    Ok(())
}

#[tauri::command]
pub async fn set_meeting_detection_enabled<R: Runtime>(
    app: AppHandle<R>,
    enabled: bool,
) -> Result<(), String> {
    let state = app
        .try_state::<MeetingDetectionState>()
        .ok_or("MeetingDetectionState not initialized")?;
    state.set_enabled(enabled);
    info!("Meeting detection set to: {}", enabled);
    Ok(())
}

#[tauri::command]
pub async fn get_meeting_detection_enabled<R: Runtime>(
    app: AppHandle<R>,
) -> Result<bool, String> {
    let state = app
        .try_state::<MeetingDetectionState>()
        .ok_or("MeetingDetectionState not initialized")?;
    Ok(state.is_enabled())
}

#[tauri::command]
pub async fn current_meeting_detection<R: Runtime>(
    app: AppHandle<R>,
) -> Result<MeetingDetectionSnapshot, String> {
    let state = app
        .try_state::<MeetingDetectionState>()
        .ok_or("MeetingDetectionState not initialized")?;
    let enabled = state.is_enabled();

    if !enabled {
        return Ok(MeetingDetectionSnapshot {
            enabled,
            active_apps: Vec::new(),
        });
    }

    let mut system = System::new();
    Ok(MeetingDetectionSnapshot {
        enabled,
        active_apps: active_meeting_names(&mut system),
    })
}

/// Return the first meeting app whose process list currently has a running
/// match. Used by `participant_detection::window_capture` to scope the
/// screenshot to the correct window-owning process. `None` if no known
/// meeting app is running.
pub fn active_meeting_app() -> Option<&'static MeetingApp> {
    if has_google_meet_window() {
        return Some(&GOOGLE_MEET_APP);
    }

    let mut system = System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    for meeting_app in MEETING_APPS {
        if !has_process(&system, meeting_app.app_processes) {
            continue;
        }
        if meeting_app.display_name == "Microsoft Teams" {
            if has_teams_meeting_signal() {
                return Some(meeting_app);
            }
            continue;
        }
        if meeting_app.meeting_indicators.is_empty()
            && !has_active_meeting_window(meeting_app, &["meeting", "call"])
        {
            continue;
        }
        if !meeting_app.meeting_indicators.is_empty()
            && !has_process(&system, meeting_app.meeting_indicators)
        {
            continue;
        }
        return Some(meeting_app);
    }
    None
}
