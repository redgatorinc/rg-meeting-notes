//! Local vision-model registry + downloader.
//!
//! Mirrors the shape of `qwen_asr_engine` so the participant-detection
//! Local path has a working download flow. Actual vision inference via
//! llama.cpp is wired in a follow-up PR; this module only handles
//! registry, download, and on-disk status so the Settings UX can be
//! completed.

pub mod commands;

use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;

// -------------------------------------------------------------- registry --

pub struct VisionModelConfig {
    pub id: &'static str,
    pub display_name: &'static str,
    pub size_mb: u32,
    pub description: &'static str,
    /// One file per model for MVP. Most small vision GGUFs ship the
    /// vision projector merged into the text-model file. When we later
    /// add models that require a separate mmproj file, extend this
    /// struct with an Option<mmproj>.
    pub repo: &'static str,
    pub filename: &'static str,
}

const VISION_MODELS: &[VisionModelConfig] = &[
    VisionModelConfig {
        id: "moondream2",
        display_name: "Moondream2",
        size_mb: 2840,
        description: "Purpose-built VQA model. Runs locally on CPU.",
        repo: "moondream/moondream2-gguf",
        filename: "moondream2-text-model-f16.gguf",
    },
    VisionModelConfig {
        id: "smolvlm-instruct",
        display_name: "SmolVLM Instruct",
        size_mb: 1920,
        description: "Fast instruction-tuned VLM. Good JSON adherence.",
        repo: "ggml-org/SmolVLM-Instruct-GGUF",
        filename: "SmolVLM-Instruct-Q4_K_M.gguf",
    },
    VisionModelConfig {
        id: "phi-3.5-vision",
        display_name: "Phi-3.5 Vision",
        size_mb: 2900,
        description: "Larger model. Better accuracy for dense meeting tiles.",
        repo: "microsoft/Phi-3.5-vision-instruct-gguf",
        filename: "Phi-3.5-vision-instruct-Q4_K_M.gguf",
    },
];

fn config_for(id: &str) -> Option<&'static VisionModelConfig> {
    VISION_MODELS.iter().find(|c| c.id == id)
}

// ---------------------------------------------------------------- types --

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum ModelStatus {
    Missing,
    Downloading { progress: u8 },
    Available,
    Corrupted { file_size: u64, expected_min_size: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub size_mb: u32,
    pub description: String,
    pub status: ModelStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: u64,
    pub speed_mbps: f64,
}

// ---------------------------------------------------------------- engine --

pub struct VisionModelEngine {
    pub models_dir: PathBuf,
    active_downloads: Arc<RwLock<HashSet<String>>>,
}

impl VisionModelEngine {
    pub fn new(models_dir: PathBuf) -> Self {
        Self {
            models_dir,
            active_downloads: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    fn model_path(&self, cfg: &VisionModelConfig) -> PathBuf {
        self.models_dir.join(cfg.id).join(cfg.filename)
    }

    pub async fn discover_models(&self) -> Vec<ModelInfo> {
        let active = self.active_downloads.read().await.clone();
        VISION_MODELS
            .iter()
            .map(|cfg| {
                let path = self.model_path(cfg);
                let status = if active.contains(cfg.id) {
                    ModelStatus::Downloading { progress: 0 }
                } else if path.exists() {
                    match std::fs::metadata(&path) {
                        Ok(m) => {
                            let size = m.len();
                            let expected_min = (cfg.size_mb as u64) * 1024 * 1024 / 2;
                            if size >= expected_min {
                                ModelStatus::Available
                            } else {
                                ModelStatus::Corrupted {
                                    file_size: size,
                                    expected_min_size: expected_min,
                                }
                            }
                        }
                        Err(_) => ModelStatus::Missing,
                    }
                } else {
                    ModelStatus::Missing
                };
                ModelInfo {
                    id: cfg.id.to_string(),
                    display_name: cfg.display_name.to_string(),
                    size_mb: cfg.size_mb,
                    description: cfg.description.to_string(),
                    status,
                }
            })
            .collect()
    }

    pub async fn download(
        &self,
        model_id: &str,
        on_progress: impl Fn(DownloadProgress) + Send + 'static,
    ) -> Result<()> {
        let cfg = config_for(model_id)
            .ok_or_else(|| anyhow!("Unknown vision model: {}", model_id))?;

        {
            let active = self.active_downloads.read().await;
            if active.contains(cfg.id) {
                return Err(anyhow!("Download already in progress for: {}", cfg.id));
            }
        }
        self.active_downloads
            .write()
            .await
            .insert(cfg.id.to_string());

        let url = format!(
            "https://huggingface.co/{}/resolve/main/{}",
            cfg.repo, cfg.filename
        );
        let dest_dir = self.models_dir.join(cfg.id);
        tokio::fs::create_dir_all(&dest_dir)
            .await
            .context("create model dir")?;
        let dest = dest_dir.join(cfg.filename);

        let result = self
            .download_inner(&url, &dest, cfg.size_mb as u64 * 1024 * 1024, on_progress)
            .await;

        self.active_downloads.write().await.remove(cfg.id);
        result
    }

    async fn download_inner(
        &self,
        url: &str,
        dest: &PathBuf,
        expected_size: u64,
        on_progress: impl Fn(DownloadProgress) + Send + 'static,
    ) -> Result<()> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(3600))
            .build()?;
        let resp = client
            .get(url)
            .send()
            .await
            .context("GET model URL")?;

        if !resp.status().is_success() {
            return Err(anyhow!(
                "model URL returned {} for {}",
                resp.status(),
                url
            ));
        }

        let total = resp.content_length().unwrap_or(expected_size);
        let mut stream = resp.bytes_stream();
        let mut file = tokio::fs::File::create(dest).await.context("create file")?;
        let mut downloaded: u64 = 0;
        let started = Instant::now();
        let mut last_report = Instant::now();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("stream chunk")?;
            file.write_all(&chunk).await.context("write chunk")?;
            downloaded += chunk.len() as u64;
            if last_report.elapsed() >= Duration::from_millis(500) {
                let elapsed = started.elapsed().as_secs_f64().max(0.001);
                let speed = (downloaded as f64 / 1024.0 / 1024.0) / elapsed;
                on_progress(DownloadProgress {
                    downloaded,
                    total,
                    speed_mbps: speed,
                });
                last_report = Instant::now();
            }
        }
        file.flush().await.context("flush")?;
        on_progress(DownloadProgress {
            downloaded,
            total,
            speed_mbps: 0.0,
        });
        Ok(())
    }

    pub async fn delete(&self, model_id: &str) -> Result<()> {
        let cfg = config_for(model_id)
            .ok_or_else(|| anyhow!("Unknown vision model: {}", model_id))?;
        let path = self.model_path(cfg);
        if path.exists() {
            tokio::fs::remove_file(&path).await.context("remove file")?;
        }
        let dir = self.models_dir.join(cfg.id);
        let _ = tokio::fs::remove_dir(&dir).await;
        Ok(())
    }
}
