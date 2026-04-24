'use client';

import { DiarizationModelManager } from '@/components/diarization/DiarizationModelManager';
import {
  SettingsNotice,
  SettingsPanel,
  SettingsPanelHeader,
  SettingsTabHeader,
} from '@/components/settings/SettingsPanel';

/**
 * Settings tab for diarization. Hosts the model-pack manager (download /
 * delete the three pyannote + embedding bundles) and the in-scope catalog
 * note. The "Auto-diarize after recording" toggle lives in the General
 * tab alongside the other recording-behavior toggles — keep all
 * cross-cutting workflow toggles in one place.
 */
export function DiarizationSettings() {
  return (
    <div className="space-y-6">
      <SettingsTabHeader
        title="Diarization"
        description="Split each recording into distinct speakers. Pick a model pack below — larger packs are more accurate, smaller packs are faster on CPU."
      />

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
