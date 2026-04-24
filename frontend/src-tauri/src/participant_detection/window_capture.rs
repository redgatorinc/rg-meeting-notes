//! Capture a PNG of the active meeting app's window (and nothing else).
//!
//! Uses `xcap` which wraps per-OS capture APIs:
//!   - Windows: `PrintWindow` (works for most Electron/Qt apps; may return a
//!     blank frame for hardware-accelerated rendering when the window is
//!     fully minimized — documented limitation, WGC fallback is a follow-up).
//!   - macOS: `ScreenCaptureKit` / `CGWindowListCreateImage` (captures a
//!     window even when fully behind other windows, but NOT when minimized
//!     to the Dock — macOS releases the GPU-composited content).
//!   - Linux: X11 offscreen composition; Wayland via xdg-desktop-portal.
//!
//! Privacy: we **never** fall back to full-desktop capture. If the meeting
//! window cannot be located or captured, we return an error and let the
//! caller surface a clear message to the user.

use anyhow::{anyhow, Context, Result};
use image::{ImageBuffer, Rgba};
use std::io::Cursor;
use xcap::Window;

use crate::meeting_detector::{active_meeting_app, MeetingApp};

pub struct CapturedWindow {
    pub png_bytes: Vec<u8>,
    pub source_app: &'static str,
    pub window_title: String,
    pub width: u32,
    pub height: u32,
}

/// Find the foreground meeting app (Teams / Zoom / …), locate its largest
/// visible window owned by one of its known process names, and return a
/// PNG of that single window.
pub fn capture_active_meeting_window() -> Result<CapturedWindow> {
    let meeting_app = active_meeting_app()
        .ok_or_else(|| anyhow!("No known meeting app is currently running. Start Teams, Zoom, Meet, or another supported app first."))?;
    capture_for_app(meeting_app)
}

fn capture_for_app(meeting_app: &'static MeetingApp) -> Result<CapturedWindow> {
    let windows = Window::all().context("Failed to enumerate windows via xcap")?;

    // Candidate = owned by one of the app's process names, non-minimized
    // where possible, with non-zero area. Pick the largest — meeting apps
    // have a main window + occasional tool/picker windows we want to skip.
    let mut best: Option<(Window, u32)> = None;
    for win in windows {
        let owner_name = win
            .app_name()
            .unwrap_or_default()
            .to_lowercase();
        let title = win.title().unwrap_or_default().to_lowercase();
        let matches_process = meeting_app
            .app_processes
            .iter()
            .any(|p| owner_name.contains(p) || title.contains(p));
        if !matches_process {
            continue;
        }
        // Skip clearly-invalid windows (zero area).
        let w = win.width().unwrap_or(0);
        let h = win.height().unwrap_or(0);
        let area = w.saturating_mul(h);
        if area == 0 {
            continue;
        }
        if best.as_ref().map_or(true, |(_, a)| area > *a) {
            best = Some((win, area));
        }
    }

    let window = best.ok_or_else(|| {
        anyhow!(
            "Found the {} process but no capturable window for it. On macOS, minimized-to-Dock windows are not capturable. Bring the window to the foreground and try again.",
            meeting_app.display_name
        )
    })?;
    let title = window.title().unwrap_or_default();
    let width = window.width().unwrap_or(0);
    let height = window.height().unwrap_or(0);

    let rgba = window.capture_image().context(
        "xcap failed to capture the meeting window. On Windows this can happen if the window is minimized and its renderer did not respond; on macOS it means Screen Recording permission is missing or the window is minimized to the Dock.",
    )?;

    // Encode the RGBA buffer to PNG in memory. `rgba` is image::RgbaImage.
    let buf: ImageBuffer<Rgba<u8>, Vec<u8>> = rgba;
    let mut png_bytes = Vec::with_capacity((width as usize * height as usize) / 2);
    buf.write_to(&mut Cursor::new(&mut png_bytes), image::ImageFormat::Png)
        .context("Failed to encode captured window as PNG")?;

    Ok(CapturedWindow {
        png_bytes,
        source_app: meeting_app.display_name,
        window_title: title,
        width,
        height,
    })
}
