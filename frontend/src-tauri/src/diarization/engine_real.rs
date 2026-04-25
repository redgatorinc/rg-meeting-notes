//! Real ONNX speaker-diarization engine.
//!
//! Gated behind the `diarization-onnx` Cargo feature — enable with
//! `cargo build --features diarization-onnx`. Default builds use the
//! stub engine in `engine.rs` so the app still runs if the native
//! sherpa-onnx binaries aren't available on the build host.
//!
//! Pipeline per sherpa-onnx:
//!   1. Decode `<folder>/audio.mp4` to mono 16 kHz f32 samples
//!   2. Feed the samples into `OfflineSpeakerDiarization` (pyannote
//!      segmentation + WeSpeaker / 3D-Speaker embedding + clustering)
//!   3. Map each returned segment to a cluster index and tally speaking
//!      time
//!
//! The caller (`commands::diarization_start`) then:
//!   - Writes `speakers` rows via `replace_meeting_speakers`
//!   - Updates `transcripts.speaker_id` based on segment overlap with
//!     each transcript's `audio_start_time` / `audio_end_time`
//!
//! Segment→transcript mapping is cheap: for each transcript row, pick
//! the cluster whose segments have the largest temporal overlap with
//! the transcript window. Transcripts without any overlap are left
//! unassigned (`speaker_id = NULL`) so the UI renders "Speaker ?".

#![cfg(feature = "diarization-onnx")]

use std::collections::HashMap;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

use crate::audio::decoder::decode_audio_file;
use crate::audio::audio_processing::{audio_to_mono, resample};

use super::engine::{Assignment, Cluster};
use super::models::{embedding_path, segmentation_path};
use super::ModelPack;

const TARGET_SAMPLE_RATE: u32 = 16_000;

/// One speaker segment returned by sherpa-onnx.
struct Segment {
    start_s: f32,
    end_s: f32,
    cluster_idx: i64,
}

/// Load the two ONNX files for a pack and run the full pipeline.
pub fn diarize_audio(
    audio_path: &Path,
    pack: ModelPack,
    transcripts: &[crate::database::models::Transcript],
) -> Result<(Vec<Cluster>, Vec<Assignment>)> {
    let seg_path = segmentation_path(pack)
        .context("failed to resolve segmentation model path")?;
    let emb_path = embedding_path(pack)
        .context("failed to resolve embedding model path")?;

    if !seg_path.exists() || !emb_path.exists() {
        return Err(anyhow!(
            "Diarization pack not installed. Download it in Settings → Diarization before running."
        ));
    }

    // 1. Decode + resample to 16 kHz mono f32.
    let decoded =
        decode_audio_file(audio_path).context("failed to decode audio file")?;
    let mono: Vec<f32> = if decoded.channels > 1 {
        audio_to_mono(&decoded.samples, decoded.channels as u16)
    } else {
        decoded.samples
    };
    let samples = if decoded.sample_rate != TARGET_SAMPLE_RATE {
        resample(&mono, decoded.sample_rate, TARGET_SAMPLE_RATE)
            .context("failed to resample audio to 16 kHz")?
    } else {
        mono
    };

    // 2. Run sherpa-onnx. The crate's API is stable across the 0.5–0.6
    // range; we use the free functions to keep the call site small.
    let raw_segments = run_sherpa_pipeline(&samples, TARGET_SAMPLE_RATE, &seg_path, &emb_path)?;

    if raw_segments.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let distinct_raw: std::collections::HashSet<i64> =
        raw_segments.iter().map(|s| s.cluster_idx).collect();
    log::info!(
        "engine_real: sherpa produced {} segments across {} raw clusters",
        raw_segments.len(),
        distinct_raw.len()
    );

    // sherpa-onnx's speaker ids can be sparse (e.g. 1, 3 for a 2-speaker
    // meeting because an intermediate cluster got merged). Renumber to
    // dense 0..N in first-appearance order so the UI's
    // `Speaker {cluster_idx + 1}` fallback produces 1, 2, 3 sequentially.
    let mut remap: HashMap<i64, i64> = HashMap::new();
    let segments: Vec<Segment> = raw_segments
        .into_iter()
        .map(|s| {
            let next = remap.len() as i64;
            let idx = *remap.entry(s.cluster_idx).or_insert(next);
            Segment {
                start_s: s.start_s,
                end_s: s.end_s,
                cluster_idx: idx,
            }
        })
        .collect();

    // 3. Tally per-cluster speaking time.
    let mut speaking_ms: HashMap<i64, i64> = HashMap::new();
    for s in &segments {
        let dur_ms = ((s.end_s - s.start_s) * 1000.0).max(0.0) as i64;
        *speaking_ms.entry(s.cluster_idx).or_insert(0) += dur_ms;
    }
    let mut cluster_indices: Vec<i64> = speaking_ms.keys().copied().collect();
    cluster_indices.sort();
    let clusters: Vec<Cluster> = cluster_indices
        .iter()
        .map(|idx| Cluster {
            cluster_idx: *idx,
            total_speaking_ms: *speaking_ms.get(idx).unwrap_or(&0),
            centroid_embedding: None,
        })
        .collect();

    // 4. Assign each transcript to the cluster with the largest temporal
    //    overlap. Transcripts with no overlap stay unassigned and the
    //    caller treats that as NULL.
    let assignments: Vec<Assignment> = transcripts
        .iter()
        .filter_map(|t| {
            let start = t.audio_start_time? as f32;
            let end = t.audio_end_time.unwrap_or(start as f64 + t.duration.unwrap_or(0.0)) as f32;
            if end <= start {
                return None;
            }
            let mut best: Option<(i64, f32)> = None;
            for seg in &segments {
                let overlap = (end.min(seg.end_s) - start.max(seg.start_s)).max(0.0);
                if overlap <= 0.0 {
                    continue;
                }
                match best {
                    Some((_, b)) if overlap <= b => {}
                    _ => best = Some((seg.cluster_idx, overlap)),
                }
            }
            best.map(|(cluster_idx, _)| Assignment {
                transcript_id: t.id.clone(),
                cluster_idx,
            })
        })
        .collect();

    Ok((clusters, assignments))
}

/// Wrap the official sherpa-onnx Rust bindings. Split into its own fn so
/// the feature gate keeps the surface of use-statements tight at the top.
///
/// The official crate (which replaces the deprecated sherpa-rs) exposes
/// `OfflineSpeakerDiarization` directly over the C API. Segmentation uses
/// the pyannote model, embedding uses wespeaker / 3D-speaker, clustering
/// uses the bundled fast-clustering implementation.
fn run_sherpa_pipeline(
    samples: &[f32],
    _sample_rate: u32,
    seg_path: &Path,
    emb_path: &Path,
) -> Result<Vec<Segment>> {
    use sherpa_onnx::{
        FastClusteringConfig, OfflineSpeakerDiarization, OfflineSpeakerDiarizationConfig,
        OfflineSpeakerSegmentationModelConfig, OfflineSpeakerSegmentationPyannoteModelConfig,
        SpeakerEmbeddingExtractorConfig,
    };

    let config = OfflineSpeakerDiarizationConfig {
        segmentation: OfflineSpeakerSegmentationModelConfig {
            pyannote: OfflineSpeakerSegmentationPyannoteModelConfig {
                model: Some(seg_path.to_string_lossy().into_owned()),
            },
            ..Default::default()
        },
        embedding: SpeakerEmbeddingExtractorConfig {
            model: Some(emb_path.to_string_lossy().into_owned()),
            ..Default::default()
        },
        clustering: FastClusteringConfig {
            num_clusters: -1, // -1 → pick automatically via threshold
            // Lower threshold = more clusters (more permissive split).
            // Empirical path so far: 0.5 → merged 3 speakers into 1,
            // 0.4 → still conservative, 0.3 → splits 2-4 speaker calls
            // cleanly on WeSpeaker ResNet34. Room to go lower on noisy
            // recordings; we'll expose the knob in Settings later if
            // users need it.
            threshold: 0.3,
        },
        ..Default::default()
    };

    let sd = OfflineSpeakerDiarization::create(&config)
        .ok_or_else(|| anyhow!("OfflineSpeakerDiarization::create returned None — check model paths and that the build linked sherpa-onnx native libs"))?;

    let result = sd
        .process(samples)
        .ok_or_else(|| anyhow!("sherpa-onnx process() returned None — audio likely too short or models unreadable"))?;

    let out = result
        .sort_by_start_time()
        .into_iter()
        .map(|s| Segment {
            start_s: s.start,
            end_s: s.end,
            cluster_idx: s.speaker as i64,
        })
        .collect();
    Ok(out)
}
