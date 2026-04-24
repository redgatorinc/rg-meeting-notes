'use client';

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { DiarizationModelManager } from '@/components/diarization/DiarizationModelManager';
import {
  SettingsNotice,
  SettingsPanel,
  SettingsPanelHeader,
  SettingsTabHeader,
} from '@/components/settings/SettingsPanel';

interface DiarizationEngineInfo {
  real_engine_available: boolean;
}

/**
 * Settings tab for diarization. Hosts the model-pack manager (download /
 * delete the three pyannote + embedding bundles) and the in-scope catalog
 * note. The "Auto-diarize after recording" toggle lives in the General
 * tab alongside the other recording-behavior toggles — keep all
 * cross-cutting workflow toggles in one place.
 */
export function DiarizationSettings() {
  const [engineInfo, setEngineInfo] = useState<DiarizationEngineInfo | null>(null);

  useEffect(() => {
    let disposed = false;
    void (async () => {
      try {
        const info = await invoke<DiarizationEngineInfo>('diarization_engine_info');
        if (!disposed) setEngineInfo(info);
      } catch {
        if (!disposed) setEngineInfo({ real_engine_available: false });
      }
    })();
    return () => {
      disposed = true;
    };
  }, []);

  const stubMode = engineInfo && !engineInfo.real_engine_available;

  return (
    <div className="space-y-6">
      <SettingsTabHeader
        title="Diarization"
        description="Split each recording into distinct speakers. Pick a model pack below — larger packs are more accurate, smaller packs are faster on CPU."
      />

      {stubMode && (
        <SettingsNotice tone="warning" className="p-3 text-xs">
          <p className="text-xs text-amber-900">
            <strong>Stub engine active.</strong> This build was compiled
            without the <code>diarization-onnx</code> Cargo feature, so the
            Diarize button currently produces a simple mic/system split
            (1–2 clusters) instead of running the pyannote + embedding
            pipeline. Downloading a model pack below still works, but the
            real pipeline only runs after rebuilding with{' '}
            <code>cargo build --features diarization-onnx</code>.
          </p>
        </SettingsNotice>
      )}

      <SettingsPanel>
        <SettingsPanelHeader title="Model packs" className="mb-4" />
        <DiarizationModelManager />
      </SettingsPanel>

      <SettingsNotice tone="info" className="p-3 text-xs">
        <p className="text-xs text-blue-900">
          <strong>On the roadmap:</strong> <code>nvidia/canary-qwen-2.5b</code>{' '}
          and <code>CohereLabs/cohere-transcribe-03-2026</code> are on the
          long-term list but not shipped — Canary has no ONNX export yet, and
          the Cohere model is not yet published. Re-opens when upstream ships
          locally-runnable weights or we add a cloud-transcription path.
        </p>
      </SettingsNotice>
    </div>
  );
}
