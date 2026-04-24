//! Tauri commands for speaker diarization.
//!
//! Phase 1 wires the command surface and the stub engine so the frontend
//! can exercise the full flow — enqueue a job, poll/receive status, list
//! speakers, rename them. The stub engine runs synchronously and finishes
//! fast; when the real sherpa-onnx engine lands in a follow-up PR, the
//! `diarization_start` implementation becomes a spawned background task
//! that emits `diarization-progress` events and the stub call is gone.

use std::sync::RwLock;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Runtime, State};

use super::engine::Engine;
use super::{DiarizationStatus, ModelPack, ModelPackInfo};
use crate::database::models::Speaker;
use crate::database::repositories::speaker::{NewSpeaker, SpeakersRepository};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// In-memory status map keyed by meeting_id. Stub-phase only; the real engine
// will replace this with a proper job queue.
// ---------------------------------------------------------------------------
use std::collections::HashMap;
use std::sync::LazyLock;

static STATUS: LazyLock<RwLock<HashMap<String, DiarizationStatus>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

fn set_status(meeting_id: &str, status: DiarizationStatus) {
    if let Ok(mut map) = STATUS.write() {
        map.insert(meeting_id.to_string(), status);
    }
}

fn current_status(meeting_id: &str) -> DiarizationStatus {
    STATUS
        .read()
        .ok()
        .and_then(|m| m.get(meeting_id).cloned())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Enqueue a diarization run for the given meeting.
///
/// Phase 1: runs the stub engine synchronously and returns the fresh
/// speakers list so the UI can refresh immediately. Also emits the same
/// events the real engine will so the frontend listener code is already
/// the final shape.
#[tauri::command]
pub async fn diarization_start<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    meeting_id: String,
    pack: Option<ModelPack>,
) -> Result<Vec<Speaker>, String> {
    let pack = pack.unwrap_or_default();
    set_status(
        &meeting_id,
        DiarizationStatus::Running { progress: 0.0 },
    );
    let _ = app.emit(
        "diarization-progress",
        serde_json::json!({ "meeting_id": meeting_id, "progress": 0.0 }),
    );

    let pool = state.db_manager.pool();

    let transcripts = sqlx::query_as::<_, crate::database::models::Transcript>(
        "SELECT id, meeting_id, transcript, timestamp, summary, action_items,
                key_points, audio_start_time, audio_end_time, duration, speaker_id
         FROM transcripts
         WHERE meeting_id = ?
         ORDER BY timestamp",
    )
    .bind(&meeting_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        let msg = format!("Failed to load transcripts: {}", e);
        set_status(&meeting_id, DiarizationStatus::Error { message: msg.clone() });
        msg
    })?;

    if transcripts.is_empty() {
        set_status(
            &meeting_id,
            DiarizationStatus::Done { speaker_count: 0 },
        );
        let _ = app.emit(
            "diarization-complete",
            serde_json::json!({ "meeting_id": meeting_id, "speaker_count": 0 }),
        );
        return Ok(Vec::new());
    }

    // Stub: split by simple hash into 2 clusters for single-mic meetings,
    // or 3 if there are enough distinct rows. Real engine will decide.
    let max_clusters = if transcripts.len() >= 6 { 3 } else { 2 };
    let (clusters, assignments) = Engine::diarize(&transcripts, pack, max_clusters);

    let new_rows: Vec<NewSpeaker> = clusters
        .iter()
        .map(|c| NewSpeaker {
            cluster_idx: c.cluster_idx,
            total_speaking_ms: c.total_speaking_ms,
            centroid_embedding: c.centroid_embedding.clone(),
            embedding_model: pack.embedding_model_id().to_string(),
        })
        .collect();

    let saved = SpeakersRepository::replace_meeting_speakers(pool, &meeting_id, &new_rows)
        .await
        .map_err(|e| {
            let msg = format!("Failed to persist speakers: {}", e);
            set_status(&meeting_id, DiarizationStatus::Error { message: msg.clone() });
            msg
        })?;

    // Build cluster_idx -> speaker uuid lookup, then update transcripts.
    let mut idx_to_id = std::collections::HashMap::with_capacity(saved.len());
    for s in &saved {
        idx_to_id.insert(s.cluster_idx, s.id.clone());
    }

    for a in &assignments {
        if let Some(speaker_uuid) = idx_to_id.get(&a.cluster_idx) {
            SpeakersRepository::assign_transcript_speaker(
                pool,
                &a.transcript_id,
                Some(speaker_uuid),
            )
            .await
            .map_err(|e| format!("Failed to assign transcript speaker: {}", e))?;
        }
    }

    // Small simulated progress for the UI animation — harmless, removed
    // once real engine emits real progress.
    for p in [0.25, 0.5, 0.75, 1.0] {
        let _ = app.emit(
            "diarization-progress",
            serde_json::json!({ "meeting_id": meeting_id, "progress": p }),
        );
        tokio::time::sleep(Duration::from_millis(30)).await;
    }

    set_status(
        &meeting_id,
        DiarizationStatus::Done {
            speaker_count: saved.len() as u32,
        },
    );
    let _ = app.emit(
        "diarization-complete",
        serde_json::json!({
            "meeting_id": meeting_id,
            "speaker_count": saved.len()
        }),
    );

    Ok(saved)
}

#[tauri::command]
pub async fn diarization_status(meeting_id: String) -> Result<DiarizationStatus, String> {
    Ok(current_status(&meeting_id))
}

#[tauri::command]
pub async fn diarization_list_packs() -> Result<Vec<ModelPackInfo>, String> {
    // Phase 1: the stub engine doesn't actually install anything, so all
    // packs report `installed: true`. When the real engine lands, this
    // becomes a filesystem check under <app-data>/models/diarization/.
    Ok(vec![
        ModelPackInfo {
            pack: ModelPack::Default,
            installed: true,
            size_mb: ModelPack::Default.size_mb(),
        },
        ModelPackInfo {
            pack: ModelPack::Fast,
            installed: true,
            size_mb: ModelPack::Fast.size_mb(),
        },
        ModelPackInfo {
            pack: ModelPack::Accurate,
            installed: true,
            size_mb: ModelPack::Accurate.size_mb(),
        },
    ])
}

#[tauri::command]
pub async fn speakers_list(
    state: State<'_, AppState>,
    meeting_id: String,
) -> Result<Vec<Speaker>, String> {
    let pool = state.db_manager.pool();
    SpeakersRepository::list_for_meeting(pool, &meeting_id)
        .await
        .map_err(|e| format!("Failed to list speakers: {}", e))
}

#[tauri::command]
pub async fn speaker_rename(
    state: State<'_, AppState>,
    speaker_id: String,
    display_name: Option<String>,
) -> Result<(), String> {
    let pool = state.db_manager.pool();
    let name_ref = display_name.as_deref().filter(|s| !s.trim().is_empty());
    SpeakersRepository::rename(pool, &speaker_id, name_ref)
        .await
        .map_err(|e| format!("Failed to rename speaker: {}", e))
}
