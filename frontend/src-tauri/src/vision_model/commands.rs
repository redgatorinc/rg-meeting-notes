//! Tauri commands for the local vision-model registry and downloader.

use std::sync::LazyLock;
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, Runtime};

use super::{ModelInfo, VisionModelEngine};

static ENGINE: LazyLock<Mutex<Option<VisionModelEngine>>> =
    LazyLock::new(|| Mutex::new(None));

fn ensure_engine<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let mut guard = ENGINE.lock().map_err(|e| e.to_string())?;
    if guard.is_some() {
        return Ok(());
    }
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    let models_dir = app_data.join("models").join("vision");
    std::fs::create_dir_all(&models_dir)
        .map_err(|e| format!("Failed to create vision models dir: {}", e))?;
    *guard = Some(VisionModelEngine::new(models_dir));
    Ok(())
}

fn engine() -> Result<VisionModelEngine, String> {
    // VisionModelEngine is cheap to clone since it only holds a
    // PathBuf + an Arc; but we don't impl Clone. Re-acquiring the
    // singleton via lock on each call is fine — discover_models is
    // fast and download is long-running anyway.
    let guard = ENGINE.lock().map_err(|e| e.to_string())?;
    let e = guard
        .as_ref()
        .ok_or_else(|| "Vision model engine not initialized".to_string())?;
    // Return a cheap copy we can await without holding the mutex.
    Ok(VisionModelEngine {
        models_dir: e.models_dir.clone(),
        active_downloads: e.active_downloads.clone(),
    })
}

#[tauri::command]
pub async fn vision_models_list<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Vec<ModelInfo>, String> {
    ensure_engine(&app)?;
    Ok(engine()?.discover_models().await)
}

#[tauri::command]
pub async fn vision_model_download<R: Runtime>(
    app: AppHandle<R>,
    model_id: String,
) -> Result<(), String> {
    ensure_engine(&app)?;
    let app_for_progress = app.clone();
    let model_id_for_progress = model_id.clone();
    let result = engine()?
        .download(&model_id, move |p| {
            let progress = if p.total > 0 {
                ((p.downloaded as f64 / p.total as f64) * 100.0).min(100.0) as u8
            } else {
                0
            };
            let _ = app_for_progress.emit(
                "vision-model-download-progress",
                serde_json::json!({
                    "model_id": model_id_for_progress,
                    "progress": progress,
                    "downloaded": p.downloaded,
                    "total": p.total,
                    "speed_mbps": p.speed_mbps,
                }),
            );
        })
        .await;
    match result {
        Ok(()) => {
            let _ = app.emit(
                "vision-model-download-complete",
                serde_json::json!({ "model_id": model_id }),
            );
            Ok(())
        }
        Err(e) => {
            let msg = format!("{}", e);
            let _ = app.emit(
                "vision-model-download-error",
                serde_json::json!({ "model_id": model_id, "error": msg }),
            );
            Err(msg)
        }
    }
}

#[tauri::command]
pub async fn vision_model_delete<R: Runtime>(
    app: AppHandle<R>,
    model_id: String,
) -> Result<(), String> {
    ensure_engine(&app)?;
    engine()?
        .delete(&model_id)
        .await
        .map_err(|e| e.to_string())
}
