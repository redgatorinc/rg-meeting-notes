'use client';

import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AlertTriangle } from 'lucide-react';
import { toast } from 'sonner';
import { Switch } from './ui/switch';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from './ui/select';
import { Input } from './ui/input';
import type {
  AdapterStatusReport,
  AiSource,
  DetectionMode,
  ParticipantDetectionConfig,
} from '@/types';
import { VisionModelManager } from './VisionModelManager';
import {
  SettingsField,
  SettingsInset,
  SettingsNotice,
  SettingsPanel,
  SettingsPanelHeader,
  SettingsSubsectionTitle,
} from '@/components/settings/SettingsPanel';

/**
 * Settings → Transcription → Participants Detection.
 *
 * Backed by the `participant_config_get`/`participant_config_set`
 * Tauri commands (see `src-tauri/src/participant_detection/config.rs`).
 * Integrated (Beta) adapter statuses come from
 * `participant_adapter_statuses` and are re-polled whenever the user
 * toggles the section open.
 */
export function ParticipantDetectionSettings() {
  const [cfg, setCfg] = useState<ParticipantDetectionConfig | null>(null);
  const [statuses, setStatuses] = useState<AdapterStatusReport[]>([]);
  const [saving, setSaving] = useState(false);

  const reload = useCallback(async () => {
    try {
      const [c, s] = await Promise.all([
        invoke<ParticipantDetectionConfig>('participant_config_get'),
        invoke<AdapterStatusReport[]>('participant_adapter_statuses'),
      ]);
      setCfg(c);
      setStatuses(s);
    } catch (err) {
      console.error('participant detection: load failed', err);
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  const persist = useCallback(
    async (next: ParticipantDetectionConfig) => {
      setCfg(next);
      setSaving(true);
      try {
        await invoke('participant_config_set', { config: next });
      } catch (err) {
        toast.error(typeof err === 'string' ? err : 'Failed to save settings');
      } finally {
        setSaving(false);
      }
    },
    [],
  );

  if (!cfg) {
    return <div className="text-xs text-muted-foreground">Loading…</div>;
  }

  const update = (patch: Partial<ParticipantDetectionConfig>) =>
    persist({ ...cfg, ...patch });
  const integratedStatuses = summarizeAdapterStatuses(statuses);

  return (
    <SettingsPanel className="space-y-4">
      <div className="flex items-center justify-between gap-6">
        <div className="min-w-0 flex-1">
          <SettingsPanelHeader
            title="Participant Detection"
            description="Identify meeting participants from app integrations or AI fallback."
          />
        </div>
        <Switch
          checked={cfg.enabled}
          onCheckedChange={(v) => update({ enabled: v })}
          disabled={saving}
        />
      </div>

      {cfg.enabled && (
        <>
          <SettingsField title="Detection Mode">
            <Select
              value={cfg.mode}
              onValueChange={(v) => update({ mode: v as DetectionMode })}
            >
              <SelectTrigger className="focus:ring-1 focus:ring-blue-500 focus:border-blue-500">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="integrated_with_ai_fallback">
                  Integrated + AI fallback (recommended)
                </SelectItem>
                <SelectItem value="integrated">Integrated only</SelectItem>
                <SelectItem value="ai">AI model only</SelectItem>
              </SelectContent>
            </Select>
          </SettingsField>

          {cfg.mode !== 'integrated' && (
            <div className="space-y-4">
              <SettingsField title="AI Model">
                <Select
                  value={cfg.ai.source}
                  onValueChange={(source) =>
                    persist({
                      ...cfg,
                      ai: { ...cfg.ai, source: source as AiSource },
                    })
                  }
                >
                  <SelectTrigger className="focus:ring-1 focus:ring-blue-500 focus:border-blue-500">
                    <SelectValue placeholder="Select AI model source" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="local">Local vision model</SelectItem>
                    <SelectItem value="external">External summary model</SelectItem>
                  </SelectContent>
                </Select>
              </SettingsField>

              {cfg.ai.source === 'local' && (
                <div className="mt-6 space-y-3">
                  <VisionModelManager
                    selectedId={cfg.ai.local.model_id ?? null}
                    onSelect={(id) =>
                      persist({
                        ...cfg,
                        ai: {
                          ...cfg.ai,
                          local: { model_id: id },
                        },
                      })
                    }
                    enabled
                  />
                  <SettingsNotice tone="warning" className="flex items-start gap-2 px-3 py-2 text-xs">
                    <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                    <p>
                      Local vision inference ships in a follow-up. You can download and
                      select models now; detection with Local still errors until the
                      inference path is wired.
                    </p>
                  </SettingsNotice>
                </div>
              )}

              {cfg.ai.source === 'external' && (
                <div className="mt-6 space-y-3">
                  <label className="flex items-center gap-2 text-xs">
                    <Switch
                      checked={cfg.ai.external.same_as_summary}
                      onCheckedChange={(v) =>
                        persist({
                          ...cfg,
                          ai: {
                            ...cfg.ai,
                            external: { ...cfg.ai.external, same_as_summary: v },
                          },
                        })
                      }
                    />
                    Use the same model as Summary
                  </label>
                  {!cfg.ai.external.same_as_summary && (
                    <div className="space-y-2">
                      <Input
                        placeholder="Provider (openai, claude, custom-openai)"
                        value={cfg.ai.external.provider ?? ''}
                        onChange={(e) =>
                          persist({
                            ...cfg,
                            ai: {
                              ...cfg.ai,
                              external: {
                                ...cfg.ai.external,
                                provider: e.target.value,
                              },
                            },
                          })
                        }
                      />
                      <Input
                        placeholder="Model (e.g. gpt-4o-mini)"
                        value={cfg.ai.external.model ?? ''}
                        onChange={(e) =>
                          persist({
                            ...cfg,
                            ai: {
                              ...cfg.ai,
                              external: {
                                ...cfg.ai.external,
                                model: e.target.value,
                              },
                            },
                          })
                        }
                      />
                    </div>
                  )}
                </div>
              )}
            </div>
          )}

          {cfg.mode !== 'ai' && (
            <SettingsInset className="space-y-2">
              <SettingsSubsectionTitle>Integrated (Beta)</SettingsSubsectionTitle>
              <p className="text-[11px] text-muted-foreground">
                Reads Teams / Zoom / Meet app state directly. Fast and private but may break when
                those apps release a major update.
              </p>
              {integratedStatuses.map((s) => (
                <div key={s.id} className="grid grid-cols-[8rem_1fr] items-start gap-2 text-xs">
                  <span className="font-medium">{adapterLabel(s.id)}</span>
                  <div className="min-w-0">
                    <StatusBadge status={s.status} />
                  </div>
                </div>
              ))}
            </SettingsInset>
          )}
        </>
      )}
    </SettingsPanel>
  );
}

const ADAPTER_ORDER: AdapterStatusReport['id'][] = ['teams', 'zoom', 'meet'];

function summarizeAdapterStatuses(statuses: AdapterStatusReport[]) {
  const strongestById = new Map<AdapterStatusReport['id'], AdapterStatusReport>();

  for (const status of statuses) {
    const current = strongestById.get(status.id);
    if (!current || statusPriority(status.status) > statusPriority(current.status)) {
      strongestById.set(status.id, status);
    }
  }

  return ADAPTER_ORDER
    .map((id) => strongestById.get(id))
    .filter((status): status is AdapterStatusReport => Boolean(status));
}

function statusPriority(status: AdapterStatusReport['status']) {
  switch (status.state) {
    case 'ready':
      return 4;
    case 'error':
      return 3;
    case 'unsupported':
      return 2;
    case 'not_detected':
      return 1;
  }
}

function adapterLabel(id: AdapterStatusReport['id']) {
  switch (id) {
    case 'teams':
      return 'Microsoft Teams';
    case 'zoom':
      return 'Zoom';
    case 'meet':
      return 'Google Meet';
  }
}

function StatusBadge({ status }: { status: AdapterStatusReport['status'] }) {
  let label = '';
  let color = '';
  let title: string | undefined;

  switch (status.state) {
    case 'ready':
      label = 'Ready';
      color = 'bg-green-50 text-green-700';
      break;
    case 'not_detected':
      label = 'Not detected';
      color = 'bg-gray-100 text-gray-600';
      break;
    case 'unsupported':
      label = status.reason.toLowerCase().includes('browser extension')
        ? 'Extension required'
        : 'Unsupported';
      color = 'bg-amber-50 text-amber-700';
      title = status.reason;
      break;
    case 'error':
      label = 'Error';
      color = 'bg-red-50 text-red-700';
      title = status.message;
      break;
  }
  return (
    <span
      className={`${color} inline-flex max-w-full rounded-full px-2 py-0.5 text-[11px] font-medium`}
      title={title}
    >
      {label}
    </span>
  );
}
