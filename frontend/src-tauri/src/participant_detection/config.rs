//! Persistent configuration for the participant-detection feature.
//!
//! Stored as a single JSON blob in `participant_detection.json` via
//! `tauri-plugin-store`. The shape intentionally mirrors the frontend
//! Settings form so load/save is a straight serde round-trip.
//!
//! Includes a one-shot migration from PR #8's simple boolean consent
//! flag in `store.json` — if that flag is `true` and no new config
//! exists, we seed `enabled: true, mode: ai, ai.source: external,
//! same_as_summary: true` so the user doesn't have to re-opt-in.

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

const CONFIG_STORE: &str = "participant_detection.json";
const LEGACY_STORE: &str = "store.json";
const LEGACY_CONSENT_KEY: &str = "participant_detection_consent";
const CONFIG_KEY: &str = "config";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionMode {
    Integrated,
    Ai,
    IntegratedWithAiFallback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiSource {
    Local,
    External,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LocalAiConfig {
    /// Identifier of the model pack the user has selected (e.g.
    /// "moondream2", "smolvlm", "phi-3.5-vision"). Matches the key used
    /// by the `LocalVisionModelsDialog` frontend registry.
    #[serde(default)]
    pub model_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalAiConfig {
    /// When true, ignore `provider` / `model` / `has_api_key` and at call
    /// time proxy through whatever the Summary model config currently
    /// points at.
    pub same_as_summary: bool,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    /// Whether the user has saved an API key for this provider in the
    /// existing provider-keychain. The key itself is never stored in
    /// this config; only the flag that one exists.
    #[serde(default)]
    pub has_api_key: bool,
}

impl Default for ExternalAiConfig {
    fn default() -> Self {
        Self {
            same_as_summary: true,
            provider: None,
            model: Some("gpt-4o-mini".to_string()),
            has_api_key: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiConfig {
    pub source: AiSource,
    #[serde(default)]
    pub local: LocalAiConfig,
    #[serde(default)]
    pub external: ExternalAiConfig,
}

impl Default for AiSource {
    fn default() -> Self {
        AiSource::External
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterMethod {
    LogTail,
    A11y,
    ExtensionBridge,
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterConfig {
    pub enabled: bool,
    pub method: AdapterMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegratedConfig {
    pub enabled: bool,
    pub teams: AdapterConfig,
    pub zoom: AdapterConfig,
    pub meet: AdapterConfig,
    pub poll_interval_sec: u32,
}

impl Default for IntegratedConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            teams: AdapterConfig {
                enabled: true,
                method: AdapterMethod::LogTail,
            },
            zoom: AdapterConfig {
                enabled: true,
                method: AdapterMethod::Auto,
            },
            meet: AdapterConfig {
                enabled: false,
                method: AdapterMethod::ExtensionBridge,
            },
            poll_interval_sec: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantDetectionConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub enabled: bool,
    pub mode: DetectionMode,
    pub ai: AiConfig,
    pub integrated: IntegratedConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consent_accepted_at: Option<String>,
}

fn default_version() -> u32 {
    1
}

impl Default for ParticipantDetectionConfig {
    fn default() -> Self {
        Self {
            version: 1,
            enabled: false,
            mode: DetectionMode::IntegratedWithAiFallback,
            ai: AiConfig::default(),
            integrated: IntegratedConfig::default(),
            consent_accepted_at: None,
        }
    }
}

/// Load the current config, creating a default + running the PR #8
/// consent migration on first access.
pub fn load<R: Runtime>(app: &AppHandle<R>) -> ParticipantDetectionConfig {
    let Ok(store) = app.store(CONFIG_STORE) else {
        return ParticipantDetectionConfig::default();
    };

    if let Some(raw) = store.get(CONFIG_KEY) {
        if let Ok(cfg) = serde_json::from_value::<ParticipantDetectionConfig>(raw.clone()) {
            return cfg;
        }
        log::warn!("participant_detection config in store is malformed; falling back to default");
    }

    // Migration: PR #8's legacy boolean consent.
    let legacy_consent = app
        .store(LEGACY_STORE)
        .ok()
        .and_then(|s| s.get(LEGACY_CONSENT_KEY))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let mut cfg = ParticipantDetectionConfig::default();
    if legacy_consent {
        cfg.enabled = true;
        cfg.mode = DetectionMode::Ai;
        cfg.ai.source = AiSource::External;
        cfg.ai.external.same_as_summary = true;
        cfg.consent_accepted_at = Some(Utc::now().to_rfc3339());
        let _ = save(app, &cfg);
    }
    cfg
}

pub fn save<R: Runtime>(app: &AppHandle<R>, cfg: &ParticipantDetectionConfig) -> Result<()> {
    let store = app
        .store(CONFIG_STORE)
        .context("Failed to open participant_detection store")?;
    let value =
        serde_json::to_value(cfg).context("Failed to serialize ParticipantDetectionConfig")?;
    store.set(CONFIG_KEY, value);
    store.save().context("Failed to persist store")?;
    Ok(())
}
