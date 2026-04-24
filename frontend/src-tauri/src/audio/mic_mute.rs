// Default-capture endpoint mute detection.
//
// Windows: reflects the physical mute toggle (Teams/Zoom Mute button, Windows
//   mic privacy switch, hardware mic slider) via IAudioEndpointVolume::GetMute.
// Other platforms: returns None (not yet implemented).

/// `Some(true)` if the default capture endpoint is currently muted at the OS
/// level, `Some(false)` if audibly unmuted, `None` if the platform or hardware
/// cannot report that state.
pub fn get_default_capture_mute() -> Option<bool> {
    platform::get_default_capture_mute()
}

#[tauri::command]
pub async fn get_microphone_mute_state() -> Result<Option<bool>, String> {
    Ok(get_default_capture_mute())
}

#[cfg(target_os = "windows")]
mod platform {
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        eCapture, eCommunications, IMMDeviceEnumerator, MMDeviceEnumerator,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
    };

    pub fn get_default_capture_mute() -> Option<bool> {
        // SAFETY: All COM calls are wrapped in unsafe blocks; each `?` in the
        // Result path short-circuits to `None` on HRESULT failure.
        unsafe {
            // CoInitialize may return S_FALSE if already initialised on this
            // thread — that's still success for our purposes.
            let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
            let initialized_here = hr.is_ok();

            let result = query_mute();

            if initialized_here {
                CoUninitialize();
            }
            result
        }
    }

    unsafe fn query_mute() -> Option<bool> {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let device = enumerator
            .GetDefaultAudioEndpoint(eCapture, eCommunications)
            .ok()?;
        let endpoint_volume: IAudioEndpointVolume =
            device.Activate(CLSCTX_ALL, None).ok()?;
        let muted = endpoint_volume.GetMute().ok()?;
        Some(muted.as_bool())
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    pub fn get_default_capture_mute() -> Option<bool> {
        None
    }
}
