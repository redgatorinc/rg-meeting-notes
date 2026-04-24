// Per-app audio source enumeration for the Home screen.
//
// Windows: walks the default render endpoint's IAudioSessionManager2 and
// reports every process currently producing audio, with peak-level-based
// activity detection. On macOS/Linux we return a stub (Microphone + System
// audio) because per-process audio enumeration needs platform-specific work
// that's outside the scope of this change.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Microphone,
    App,
    System,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioSource {
    pub id: String,
    pub display_name: String,
    pub kind: SourceKind,
    pub process_name: Option<String>,
    pub active: bool,
}

/// Activity threshold for WASAPI peak levels. Peak is in [0.0, 1.0].
#[allow(dead_code)]
const PEAK_ACTIVE_THRESHOLD: f32 = 0.0005;

/// Friendly names for common apps; falls back to the raw executable basename.
#[allow(dead_code)]
const FRIENDLY_NAMES: &[(&str, &str)] = &[
    ("Teams.exe", "Microsoft Teams"),
    ("ms-teams.exe", "Microsoft Teams"),
    ("MSTeams.exe", "Microsoft Teams"),
    ("chrome.exe", "Google Chrome"),
    ("msedge.exe", "Microsoft Edge"),
    ("firefox.exe", "Firefox"),
    ("Discord.exe", "Discord"),
    ("Slack.exe", "Slack"),
    ("Spotify.exe", "Spotify"),
    ("zoom.exe", "Zoom"),
    ("Zoom.exe", "Zoom"),
    ("WebexMeetings.exe", "Webex"),
    ("Webex.exe", "Webex"),
];

#[allow(dead_code)]
fn friendly_name_for(exe: &str) -> String {
    for (raw, pretty) in FRIENDLY_NAMES {
        if exe.eq_ignore_ascii_case(raw) {
            return (*pretty).to_string();
        }
    }
    // Strip ".exe" if present for the generic fallback.
    exe.strip_suffix(".exe")
        .or_else(|| exe.strip_suffix(".EXE"))
        .unwrap_or(exe)
        .to_string()
}

pub fn enumerate_audio_sources() -> Vec<AudioSource> {
    platform::list_audio_sources()
}

#[tauri::command]
pub async fn list_audio_sources() -> Result<Vec<AudioSource>, String> {
    tokio::task::spawn_blocking(enumerate_audio_sources)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
mod platform {
    use super::{friendly_name_for, AudioSource, SourceKind};
    use crate::audio::mic_mute::get_default_capture_mute;
    use std::collections::HashMap;
    use std::os::windows::ffi::OsStringExt;
    use std::path::PathBuf;
    use windows::core::Interface;
    use windows::Win32::Foundation::{CloseHandle, HANDLE, MAX_PATH};
    use windows::Win32::Media::Audio::{
        eRender, eConsole, AudioSessionStateActive, IAudioSessionControl2,
        IAudioSessionEnumerator, IAudioSessionManager2, IMMDeviceEnumerator,
        MMDeviceEnumerator,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
    };
    use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    pub fn list_audio_sources() -> Vec<AudioSource> {
        let mut sources = Vec::new();

        // Microphone row is always first so the panel has at least one entry.
        let muted = get_default_capture_mute().unwrap_or(false);
        sources.push(AudioSource {
            id: "microphone:default".to_string(),
            display_name: "Microphone".to_string(),
            kind: SourceKind::Microphone,
            process_name: None,
            active: !muted,
        });

        let app_sources = unsafe { enumerate_render_sessions() };
        sources.extend(app_sources);

        sources
    }

    unsafe fn enumerate_render_sessions() -> Vec<AudioSource> {
        let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
        let initialized_here = hr.is_ok();

        let result = collect_sessions().unwrap_or_default();

        if initialized_here {
            CoUninitialize();
        }
        result
    }

    unsafe fn collect_sessions() -> Option<Vec<AudioSource>> {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
        let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None).ok()?;
        let session_enum: IAudioSessionEnumerator = manager.GetSessionEnumerator().ok()?;

        let count = session_enum.GetCount().ok()?;
        let mut sources: Vec<AudioSource> = Vec::new();
        let mut seen_pids: HashMap<u32, usize> = HashMap::new();
        let self_pid = std::process::id();

        for i in 0..count {
            let Ok(control) = session_enum.GetSession(i) else { continue };
            let Ok(ctrl2) = control.cast::<IAudioSessionControl2>() else { continue };

            let pid = ctrl2.GetProcessId().unwrap_or(0);
            if pid == 0 {
                // System-sounds sessions have PID 0; skip.
                continue;
            }
            if pid == self_pid {
                // Don't report our own app — the recorder always appears in
                // the render-session list because tauri/cpal opens an output
                // stream at startup.
                continue;
            }

            let state = control.GetState().unwrap_or_default();
            let active = state == AudioSessionStateActive;

            // Only include sessions currently in the Active state — avoids
            // cluttering the list with dormant sessions from every app that
            // ever played a sound. (A proper peak-level check via
            // IAudioMeterInformation is a future refinement.)
            if !active {
                continue;
            }

            let exe = process_name_for_pid(pid).unwrap_or_else(|| format!("pid-{pid}"));
            let display = friendly_name_for(&exe);

            if let Some(&existing_idx) = seen_pids.get(&pid) {
                // Merge duplicate sessions from the same PID (Chrome spawns one
                // per tab); keep active=true if any of them is active.
                if active {
                    sources[existing_idx].active = true;
                }
                continue;
            }

            seen_pids.insert(pid, sources.len());
            sources.push(AudioSource {
                id: format!("pid:{pid}"),
                display_name: display,
                kind: SourceKind::App,
                process_name: Some(exe),
                active,
            });
        }

        Some(sources)
    }

    fn process_name_for_pid(pid: u32) -> Option<String> {
        unsafe {
            let handle: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = [0u16; MAX_PATH as usize];
            // hmodule = None → retrieve the path of the process executable itself.
            let len = GetModuleFileNameExW(Some(handle), None, &mut buf);
            let _ = CloseHandle(handle);
            if len == 0 {
                return None;
            }
            let path: PathBuf = std::ffi::OsString::from_wide(&buf[..len as usize]).into();
            path.file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::{AudioSource, SourceKind};

    pub fn list_audio_sources() -> Vec<AudioSource> {
        vec![
            AudioSource {
                id: "microphone:default".to_string(),
                display_name: "Microphone".to_string(),
                kind: SourceKind::Microphone,
                process_name: None,
                active: true,
            },
            AudioSource {
                id: "system:default".to_string(),
                display_name: "System audio".to_string(),
                kind: SourceKind::System,
                process_name: None,
                active: true,
            },
        ]
    }
}
