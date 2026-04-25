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
use super::models::{self, DiarizationModelInfo};
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

    // Stage 1 — transcripts loaded, about to start audio decode + inference.
    let _ = app.emit(
        "diarization-progress",
        serde_json::json!({ "meeting_id": meeting_id, "progress": 0.1 }),
    );

    // Real ONNX pipeline when the `diarization-onnx` feature is on AND
    // we can resolve the meeting's audio file. Falls back to the stub
    // if either check fails — keeps existing recordings without a saved
    // audio.mp4 still able to produce the live-mic/live-system split
    // the stub provides.
    //
    // The real engine's sd.process() is one opaque blocking call we
    // can't instrument, so we fake mid-flight progress with a ticker
    // task: every 2 s it bumps the UI a tick until the engine returns.
    // Without this the progress bar would sit at 10 % for the entire
    // 15-30 s inference and look hung.
    let (clusters, assignments) = {
        let app_ticker = app.clone();
        let meeting_ticker = meeting_id.clone();
        let ticker_cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let ticker_cancel_clone = ticker_cancel.clone();
        let ticker = tokio::spawn(async move {
            // Climb from 0.15 → 0.80 over ~30 s, then hold.
            let steps: &[f64] = &[0.15, 0.25, 0.35, 0.45, 0.55, 0.65, 0.75, 0.80];
            for p in steps {
                if ticker_cancel_clone.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                let _ = app_ticker.emit(
                    "diarization-progress",
                    serde_json::json!({ "meeting_id": meeting_ticker, "progress": p }),
                );
                tokio::time::sleep(Duration::from_millis(2500)).await;
            }
        });

        log::info!(
            "diarization_start: running real engine (pack={:?}) for meeting {}",
            pack,
            meeting_id
        );
        let real_output = real_engine_run(pool, &meeting_id, pack, &transcripts).await;

        ticker_cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        ticker.abort();

        match real_output {
            Some(result) => {
                log::info!(
                    "diarization_start: real engine produced {} clusters / {} assignments",
                    result.0.len(),
                    result.1.len()
                );
                result
            }
            None => {
                log::info!("diarization_start: falling back to stub engine");
                let max_clusters = if transcripts.len() >= 6 { 3 } else { 2 };
                Engine::diarize(&transcripts, pack, max_clusters)
            }
        }
    };

    let _ = app.emit(
        "diarization-progress",
        serde_json::json!({ "meeting_id": meeting_id, "progress": 0.85 }),
    );

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

    // Fire the three name-identification passes against the fresh clusters.
    // Best-effort: individual failures are logged but don't fail the
    // command. Candidates are written to `speaker_name_candidates`; the
    // frontend listens for `diarization-name-candidates-ready`.
    run_name_pipeline(pool, &app, &meeting_id, &transcripts, &saved).await;

    Ok(saved)
}

async fn run_name_pipeline<R: Runtime>(
    pool: &sqlx::SqlitePool,
    app: &AppHandle<R>,
    meeting_id: &str,
    transcripts: &[crate::database::models::Transcript],
    speakers: &[Speaker],
) {
    // Clear stale candidates from a prior run for this meeting so the
    // approval panel only shows fresh suggestions.
    if let Err(e) =
        crate::database::repositories::speaker::SpeakerNameCandidatesRepository::clear_for_meeting(
            pool, meeting_id,
        )
        .await
    {
        log::warn!("name_pipeline: failed to clear prior candidates: {}", e);
    }

    // speaker_id (UUID) -> cluster_idx map, consumed by every pass.
    let mut sid_to_cluster: std::collections::HashMap<String, i64> =
        std::collections::HashMap::with_capacity(speakers.len());
    for s in speakers {
        sid_to_cluster.insert(s.id.clone(), s.cluster_idx);
    }

    // Cue parser — pure CPU, fast, deterministic.
    let cue_candidates =
        super::cue_parser::extract_candidates(transcripts, &sid_to_cluster);

    // LLM pass — dispatches through the summary pipeline's provider
    // surface, so it inherits every provider the user has already
    // configured (OpenAI / Claude / Groq / Ollama / BuiltInAI / …).
    let llm_candidates = super::llm_namer::extract_candidates(
        app,
        pool,
        speakers,
        &sid_to_cluster,
        transcripts,
    )
    .await;

    // Adapter pass — reads `meeting_participants` captured at recording start.
    let adapter_candidates =
        super::adapter_names::extract_candidates(pool, meeting_id, speakers).await;

    let total =
        cue_candidates.len() + llm_candidates.len() + adapter_candidates.len();
    for (cands, source) in [
        (cue_candidates, "cue_parser"),
        (llm_candidates, "llm"),
        (adapter_candidates, "adapter"),
    ] {
        for c in cands {
            if let Err(e) =
                crate::database::repositories::speaker::SpeakerNameCandidatesRepository::insert(
                    pool,
                    meeting_id,
                    c.cluster_idx,
                    &c.name,
                    source,
                    c.confidence,
                )
                .await
            {
                log::warn!("name_pipeline: failed to insert {} candidate: {}", source, e);
            }
        }
    }

    log::info!(
        "name_pipeline: wrote {} candidates for meeting {}",
        total,
        meeting_id
    );

    let _ = app.emit(
        "diarization-name-candidates-ready",
        serde_json::json!({
            "meeting_id": meeting_id,
            "candidate_count": total,
        }),
    );
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

// ---------------------------------------------------------------------------
// Model pack management — list / download / delete. Files land under
// <AppData>/models/diarization/<pack-id>/. The real ONNX inference pipeline
// consumes them; until that pipeline ships the engine stub ignores them and
// the model manager UI is still useful to pre-download packs.
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn diarization_models_list() -> Result<Vec<DiarizationModelInfo>, String> {
    Ok(models::list_info())
}

/// Engine-capability probe for the Settings UI. Lets the tab render a
/// "stub mode" notice when the real ONNX pipeline isn't built in.
#[derive(serde::Serialize)]
pub struct DiarizationEngineInfo {
    /// True when the Cargo feature `diarization-onnx` is enabled. When
    /// false, `diarization_start` always falls through to the stub.
    pub real_engine_available: bool,
}

#[tauri::command]
pub async fn diarization_engine_info() -> Result<DiarizationEngineInfo, String> {
    Ok(DiarizationEngineInfo {
        real_engine_available: cfg!(feature = "diarization-onnx"),
    })
}

#[tauri::command]
pub async fn diarization_model_download<R: Runtime>(
    app: AppHandle<R>,
    pack: ModelPack,
) -> Result<(), String> {
    models::download_pack(app.clone(), pack).await.map_err(|e| {
        let _ = app.emit(
            "diarization-model-download-error",
            serde_json::json!({
                "pack_id": models::pack_spec(pack).id,
                "error": e.to_string(),
            }),
        );
        format!("Download failed: {}", e)
    })
}

#[tauri::command]
pub async fn diarization_model_delete(pack: ModelPack) -> Result<(), String> {
    models::delete_pack(pack)
        .await
        .map_err(|e| format!("Delete failed: {}", e))
}

// ---------------------------------------------------------------------------
// Name-candidate review surface
// ---------------------------------------------------------------------------

use crate::database::repositories::speaker::{
    SpeakerNameCandidateRow, SpeakerNameCandidatesRepository,
};

#[tauri::command]
pub async fn diarization_name_candidates(
    state: State<'_, AppState>,
    meeting_id: String,
) -> Result<Vec<SpeakerNameCandidateRow>, String> {
    let pool = state.db_manager.pool();
    SpeakerNameCandidatesRepository::list_for_meeting(pool, &meeting_id)
        .await
        .map_err(|e| format!("Failed to list candidates: {}", e))
}

/// Apply user-approved names. `assignments` is `{cluster_idx -> display_name}`.
/// An empty string clears any prior name back to `Speaker N`. Clears the
/// candidates table for this meeting once applied.
#[tauri::command]
pub async fn diarization_apply_names(
    state: State<'_, AppState>,
    meeting_id: String,
    assignments: std::collections::HashMap<i64, String>,
) -> Result<(), String> {
    let pool = state.db_manager.pool();
    let speakers = SpeakersRepository::list_for_meeting(pool, &meeting_id)
        .await
        .map_err(|e| format!("Failed to load speakers: {}", e))?;

    for s in &speakers {
        if let Some(name) = assignments.get(&s.cluster_idx) {
            let trimmed = name.trim();
            let value = if trimmed.is_empty() { None } else { Some(trimmed) };
            SpeakersRepository::rename(pool, &s.id, value)
                .await
                .map_err(|e| format!("Failed to rename speaker {}: {}", s.id, e))?;
        }
    }

    if let Err(e) =
        SpeakerNameCandidatesRepository::clear_for_meeting(pool, &meeting_id).await
    {
        log::warn!(
            "apply_names: failed to clear candidates for {}: {}",
            meeting_id,
            e
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Real-engine bridge. Gated behind the `diarization-onnx` Cargo feature so
// default builds still compile without the sherpa-rs native binaries.
// ---------------------------------------------------------------------------

#[cfg(feature = "diarization-onnx")]
async fn real_engine_run(
    pool: &sqlx::SqlitePool,
    meeting_id: &str,
    pack: ModelPack,
    transcripts: &[crate::database::models::Transcript],
) -> Option<(Vec<super::engine::Cluster>, Vec<super::engine::Assignment>)> {
    // Look up the meeting's recording folder; without it we can't decode.
    let folder_path: Option<String> = sqlx::query_scalar(
        "SELECT folder_path FROM meetings WHERE id = ?",
    )
    .bind(meeting_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .flatten();

    let Some(folder) = folder_path else {
        log::info!("real_engine: meeting has no folder_path, falling back to stub");
        return None;
    };
    let audio_path = std::path::PathBuf::from(&folder).join("audio.mp4");
    if !audio_path.is_file() {
        log::info!(
            "real_engine: {} missing, falling back to stub",
            audio_path.display()
        );
        return None;
    }

    // Heavy work — run on a blocking pool so we don't stall the tokio
    // runtime with pyannote's synchronous ONNX pipeline.
    let audio_path_owned = audio_path.clone();
    let transcripts_owned = transcripts.to_vec();
    let joined = tokio::task::spawn_blocking(move || {
        super::engine_real::diarize_audio(&audio_path_owned, pack, &transcripts_owned)
    })
    .await;

    match joined {
        Ok(Ok(result)) => Some(result),
        Ok(Err(e)) => {
            log::warn!("real_engine: pipeline failed, falling back to stub: {}", e);
            None
        }
        Err(join_err) => {
            log::warn!(
                "real_engine: blocking task panicked ({}), falling back to stub",
                join_err
            );
            None
        }
    }
}

#[cfg(not(feature = "diarization-onnx"))]
async fn real_engine_run(
    _pool: &sqlx::SqlitePool,
    _meeting_id: &str,
    _pack: ModelPack,
    _transcripts: &[crate::database::models::Transcript],
) -> Option<(Vec<super::engine::Cluster>, Vec<super::engine::Assignment>)> {
    None
}
