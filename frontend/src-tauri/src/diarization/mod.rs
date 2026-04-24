//! Speaker diarization scaffolding.
//!
//! Phase 1 (this commit) ships the storage + Tauri command shape so the
//! frontend Speakers panel can be built end-to-end against real IPC. The
//! underlying diarization engine is a deliberately simple **heuristic stub**
//! today: it splits the transcript rows into a fixed set of clusters by
//! timestamp modulo. Real `sherpa-onnx` integration lands in a follow-up PR.
//!
//! The wire format (types, events, DB schema, commands) is already the shape
//! Phase 2 (online detection) and Phase 3 (cross-meeting voiceprints) will
//! consume, so the UI written against these types does not need to change
//! when the real engine wires in.

pub mod adapter_names;
pub mod commands;
pub mod cue_parser;
pub mod engine;
#[cfg(feature = "diarization-onnx")]
pub mod engine_real;
pub mod llm_namer;
pub mod models;

use serde::{Deserialize, Serialize};

/// Model pack the user can choose between. Accuracy vs size tradeoff.
/// Phase 1 treats these as labels only — the stub engine is the same for
/// all three. Once sherpa-onnx is wired in, each pack points at a specific
/// (segmentation-model, embedding-model) ONNX pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelPack {
    /// pyannote-seg-3.0 + 3D-Speaker ERes2Net base (~46 MB total).
    Default,
    /// pyannote-seg-3.0 + CAM++ (~36 MB total).
    Fast,
    /// pyannote-seg-3.0 + WeSpeaker ResNet293 (~121 MB total).
    Accurate,
}

impl Default for ModelPack {
    fn default() -> Self {
        ModelPack::Default
    }
}

impl ModelPack {
    pub fn size_mb(self) -> u32 {
        match self {
            ModelPack::Default => 46,
            ModelPack::Fast => 36,
            ModelPack::Accurate => 121,
        }
    }

    /// Identifier used by the Phase 1 stub as the `embedding_model` column
    /// value. When the real engine wires in this will become the ONNX model
    /// name so centroids from different packs are never compared by cosine.
    pub fn embedding_model_id(self) -> &'static str {
        match self {
            ModelPack::Default => "stub-default",
            ModelPack::Fast => "stub-fast",
            ModelPack::Accurate => "stub-accurate",
        }
    }
}

/// Info about a single model pack for the Settings UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPackInfo {
    pub pack: ModelPack,
    pub installed: bool,
    pub size_mb: u32,
}

/// Current state of a diarization job for a given meeting.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum DiarizationStatus {
    /// Never run for this meeting (or cleared).
    Idle,
    /// A model pack download is in flight.
    Downloading { progress: f32 },
    /// Actively diarizing. `progress` is 0..1.
    Running { progress: f32 },
    /// Finished. `speaker_count` is the number of distinct clusters found.
    Done { speaker_count: u32 },
    /// Job failed. UI shows a toast with `message`.
    Error { message: String },
}

impl Default for DiarizationStatus {
    fn default() -> Self {
        DiarizationStatus::Idle
    }
}
