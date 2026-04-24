//! Phase 1 stub diarizer.
//!
//! Groups existing transcript rows into a fixed number of pseudo-clusters
//! based on a stable hash of the transcript text so the UI can exercise
//! the full rename / re-run / model-pack flow end-to-end with zero ML
//! dependencies. Real sherpa-onnx integration replaces this module in the
//! next PR; everything outside this file consumes `StubCluster` via the
//! `Engine::diarize` return value, which already matches the shape the
//! real engine will produce.

use super::ModelPack;
use crate::database::models::Transcript;

/// One cluster produced by a diarization run.
pub struct Cluster {
    pub cluster_idx: i64,
    pub total_speaking_ms: i64,
    /// Phase 1 leaves this empty. Phase 2 will populate it with an
    /// L2-normalized f32 embedding encoded as little-endian bytes.
    pub centroid_embedding: Option<Vec<u8>>,
}

/// Per-transcript assignment. Cluster indices line up with
/// [`Cluster::cluster_idx`] in the sibling `Vec`.
pub struct Assignment {
    pub transcript_id: String,
    pub cluster_idx: i64,
}

pub struct Engine;

impl Engine {
    /// Assign each transcript row to one of at most `max_clusters` clusters.
    ///
    /// Phase 1 stub strategy: hash the first character of the transcript
    /// text (fallback: the row index) into 0..max_clusters. Gives a
    /// deterministic but useful-looking split for UI development. Two
    /// speakers is the most common meeting shape, so `max_clusters=2` is a
    /// decent default until the real engine lands.
    pub fn diarize(
        transcripts: &[Transcript],
        pack: ModelPack,
        _max_clusters: usize,
    ) -> (Vec<Cluster>, Vec<Assignment>) {
        let _ = pack; // reserved for future pack-specific tuning

        // Phase-1 stub strategy: if the live pipeline already tagged
        // transcripts with `live-mic` or `live-system`, trust that —
        // one cluster for "You" (mic) and one for "Remote" (system).
        // That's an honest 1- or 2-speaker split, not the 3-way fake
        // hash we had before. If no live tags are present (e.g. the
        // recording was imported from a file) fall back to a single
        // cluster. Real multi-speaker splitting lands with sherpa-onnx.
        const MIC_IDX: i64 = 0;
        const SYSTEM_IDX: i64 = 1;

        let mut mic_ms: i64 = 0;
        let mut system_ms: i64 = 0;
        let mut uncategorized_ms: i64 = 0;
        let mut assignments = Vec::with_capacity(transcripts.len());

        for t in transcripts {
            let duration_ms = t
                .duration
                .map(|d| (d * 1000.0).round().max(0.0) as i64)
                .unwrap_or(0);
            let bucket = match t.speaker_id.as_deref() {
                Some("live-mic") => {
                    mic_ms += duration_ms;
                    MIC_IDX
                }
                Some("live-system") => {
                    system_ms += duration_ms;
                    SYSTEM_IDX
                }
                _ => {
                    uncategorized_ms += duration_ms;
                    MIC_IDX // lump unknowns into "You" so we don't invent a phantom speaker
                }
            };
            assignments.push(Assignment {
                transcript_id: t.id.clone(),
                cluster_idx: bucket,
            });
        }

        let mut clusters = Vec::new();
        if mic_ms + uncategorized_ms > 0 {
            clusters.push(Cluster {
                cluster_idx: MIC_IDX,
                total_speaking_ms: mic_ms + uncategorized_ms,
                centroid_embedding: None,
            });
        }
        if system_ms > 0 {
            clusters.push(Cluster {
                cluster_idx: SYSTEM_IDX,
                total_speaking_ms: system_ms,
                centroid_embedding: None,
            });
        }

        (clusters, assignments)
    }
}
