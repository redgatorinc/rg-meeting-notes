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
        max_clusters: usize,
    ) -> (Vec<Cluster>, Vec<Assignment>) {
        let n = max_clusters.max(1);
        let _ = pack; // reserved for future pack-specific tuning

        let mut totals: Vec<i64> = vec![0; n];
        let mut assignments = Vec::with_capacity(transcripts.len());

        for (idx, t) in transcripts.iter().enumerate() {
            let bucket = cluster_for(&t.transcript, idx, n) as i64;
            let duration_ms = t
                .duration
                .map(|d| (d * 1000.0).round().max(0.0) as i64)
                .unwrap_or(0);
            totals[bucket as usize] += duration_ms;
            assignments.push(Assignment {
                transcript_id: t.id.clone(),
                cluster_idx: bucket,
            });
        }

        let clusters: Vec<Cluster> = totals
            .into_iter()
            .enumerate()
            .filter(|(_, ms)| *ms > 0) // drop empty clusters so UI doesn't show phantoms
            .map(|(idx, total_speaking_ms)| Cluster {
                cluster_idx: idx as i64,
                total_speaking_ms,
                centroid_embedding: None,
            })
            .collect();

        (clusters, assignments)
    }
}

fn cluster_for(text: &str, row_idx: usize, n: usize) -> usize {
    let ch_hash = text
        .chars()
        .next()
        .map(|c| c as usize)
        .unwrap_or(row_idx);
    ch_hash % n
}
