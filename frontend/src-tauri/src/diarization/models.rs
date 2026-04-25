//! Diarization model packs — registry + download + filesystem management.
//!
//! Each pack is (segmentation model) + (speaker-embedding model). The
//! segmentation model is shared across all three packs today
//! (pyannote-segmentation-3.0 pre-converted to ONNX by csukuangfj); the
//! embedding model differs. All files are fetched from Hugging Face public
//! repos.
//!
//! Files land under `<AppData>/models/diarization/<pack_id>/`:
//!   ├── segmentation.onnx
//!   └── embedding.onnx
//!
//! The real ONNX inference pipeline (`pipeline.rs`) loads these via
//! sherpa-onnx once that integration lands. Until then, `is_installed` is
//! the source of truth for the UI model manager, and the inference stub in
//! `engine.rs` is unaware of downloads.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tauri::Emitter;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use super::ModelPack;

/// Static URL pair for one pack.
#[derive(Debug, Clone, Copy)]
pub struct PackSpec {
    pub id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub segmentation_url: &'static str,
    pub embedding_url: &'static str,
    pub embedding_model_id: &'static str,
    pub total_size_mb: u32,
}

/// Serializable projection used by the frontend model manager.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiarizationModelInfo {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub size_mb: u32,
    pub installed: bool,
}

/// Aggregate download progress across the two files in a pack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiarizationDownloadProgress {
    pub pack_id: String,
    pub stage: String, // "segmentation" | "embedding"
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percent: u8,
}

/// Canonical URLs — csukuangfj publishes pre-converted ONNX bundles for
/// sherpa-onnx that are small, CPU-friendly, and well-tested. Each pack's
/// segmentation file is the same — only the embedding network changes.
const SEGMENTATION_URL: &str =
    "https://huggingface.co/csukuangfj/sherpa-onnx-pyannote-segmentation-3-0/resolve/main/model.onnx?download=true";

pub fn pack_spec(pack: ModelPack) -> PackSpec {
    // Embedding URLs point at the k2-fsa/sherpa-onnx GitHub release
    // `speaker-recongition-models` (sic — upstream typo, tag is final).
    // Csukuangfj's individual HF repos for WeSpeaker / 3D-Speaker have
    // become gated (401 for anonymous clients), whereas GitHub release
    // assets stay public.
    match pack {
        ModelPack::Default => PackSpec {
            id: "default",
            display_name: "Default (pyannote + WeSpeaker)",
            description:
                "pyannote-segmentation-3.0 + WeSpeaker ResNet34-VoxCeleb. Balanced accuracy and size, recommended for most meetings.",
            segmentation_url: SEGMENTATION_URL,
            embedding_url:
                "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-recongition-models/wespeaker_en_voxceleb_resnet34.onnx",
            embedding_model_id: "wespeaker_en_voxceleb_resnet34",
            total_size_mb: 46,
        },
        ModelPack::Fast => PackSpec {
            id: "fast",
            display_name: "Fast (pyannote + 3D-Speaker CAM++)",
            description:
                "pyannote-segmentation-3.0 + 3D-Speaker CAM++. Smallest footprint, fastest inference on CPU.",
            segmentation_url: SEGMENTATION_URL,
            embedding_url:
                "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-recongition-models/3dspeaker_speech_campplus_sv_en_voxceleb_16k.onnx",
            embedding_model_id: "3dspeaker_campplus_en_voxceleb",
            total_size_mb: 36,
        },
        ModelPack::Accurate => PackSpec {
            id: "accurate",
            display_name: "Accurate (pyannote + WeSpeaker ResNet293)",
            description:
                "pyannote-segmentation-3.0 + WeSpeaker ResNet293. Highest diarization accuracy, larger download and slower inference.",
            segmentation_url: SEGMENTATION_URL,
            embedding_url:
                "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-recongition-models/wespeaker_en_voxceleb_resnet293_LM.onnx",
            embedding_model_id: "wespeaker_en_voxceleb_resnet293",
            total_size_mb: 121,
        },
    }
}

/// Root directory holding every pack. `<AppData>/models/diarization/`.
pub fn diarization_root() -> Result<PathBuf> {
    let data = dirs::data_dir().ok_or_else(|| anyhow!("No data dir"))?;
    Ok(data.join("com.meetily.ai").join("models").join("diarization"))
}

pub fn pack_dir(pack: ModelPack) -> Result<PathBuf> {
    Ok(diarization_root()?.join(pack_spec(pack).id))
}

pub fn segmentation_path(pack: ModelPack) -> Result<PathBuf> {
    Ok(pack_dir(pack)?.join("segmentation.onnx"))
}

pub fn embedding_path(pack: ModelPack) -> Result<PathBuf> {
    Ok(pack_dir(pack)?.join("embedding.onnx"))
}

pub fn is_installed(pack: ModelPack) -> bool {
    let Ok(seg) = segmentation_path(pack) else { return false };
    let Ok(emb) = embedding_path(pack) else { return false };
    seg.metadata()
        .map(|m| m.is_file() && m.len() > 1024)
        .unwrap_or(false)
        && emb
            .metadata()
            .map(|m| m.is_file() && m.len() > 1024)
            .unwrap_or(false)
}

pub fn list_info() -> Vec<DiarizationModelInfo> {
    [ModelPack::Default, ModelPack::Fast, ModelPack::Accurate]
        .into_iter()
        .map(|p| {
            let spec = pack_spec(p);
            DiarizationModelInfo {
                id: spec.id.to_string(),
                display_name: spec.display_name.to_string(),
                description: spec.description.to_string(),
                size_mb: spec.total_size_mb,
                installed: is_installed(p),
            }
        })
        .collect()
}

/// Stream one URL to a destination file. Emits progress every ~1 % change
/// or every 250 ms, whichever comes first.
async fn stream_to_file<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    pack: ModelPack,
    stage: &str,
    url: &str,
    dest: &PathBuf,
) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(600))
        .build()?;

    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!("HTTP {} for {}", resp.status(), url));
    }
    let total_bytes = resp.content_length().unwrap_or(0);

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).await?;
    }
    let tmp = dest.with_extension("onnx.part");
    let file = fs::File::create(&tmp).await?;
    let mut writer = tokio::io::BufWriter::new(file);

    let mut downloaded: u64 = 0;
    let mut last_pct: u8 = 0;
    let mut last_emit = std::time::Instant::now();

    let mut stream = resp.bytes_stream();
    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        writer.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;

        let pct = if total_bytes > 0 {
            ((downloaded as f64 / total_bytes as f64) * 100.0).min(100.0) as u8
        } else {
            0
        };
        let now = std::time::Instant::now();
        if pct != last_pct || now.duration_since(last_emit).as_millis() > 250 {
            let _ = app.emit(
                "diarization-model-download-progress",
                DiarizationDownloadProgress {
                    pack_id: pack_spec(pack).id.to_string(),
                    stage: stage.to_string(),
                    downloaded_bytes: downloaded,
                    total_bytes,
                    percent: pct,
                },
            );
            last_pct = pct;
            last_emit = now;
        }
    }

    writer.flush().await?;
    drop(writer);
    fs::rename(&tmp, dest).await?;
    Ok(())
}

pub async fn download_pack<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    pack: ModelPack,
) -> Result<()> {
    let spec = pack_spec(pack);
    let seg_dest = segmentation_path(pack)?;
    let emb_dest = embedding_path(pack)?;

    // Skip already-present files so re-downloading a partially-installed
    // pack only fetches the missing half.
    let seg_ok = seg_dest.metadata().map(|m| m.len() > 1024).unwrap_or(false);
    let emb_ok = emb_dest.metadata().map(|m| m.len() > 1024).unwrap_or(false);

    if !seg_ok {
        stream_to_file(&app, pack, "segmentation", spec.segmentation_url, &seg_dest).await?;
    }
    if !emb_ok {
        stream_to_file(&app, pack, "embedding", spec.embedding_url, &emb_dest).await?;
    }

    let _ = app.emit(
        "diarization-model-download-complete",
        serde_json::json!({ "pack_id": spec.id }),
    );
    Ok(())
}

pub async fn delete_pack(pack: ModelPack) -> Result<()> {
    let dir = pack_dir(pack)?;
    if dir.is_dir() {
        fs::remove_dir_all(&dir).await?;
    }
    Ok(())
}
