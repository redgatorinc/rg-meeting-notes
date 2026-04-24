'use client';

import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import { Label } from './ui/label';
import { Switch } from './ui/switch';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from './ui/select';
import { Input } from './ui/input';
import type {
  AdapterStatusReport,
  DetectionMode,
  ParticipantDetectionConfig,
} from '@/types';
import { VisionModelManager } from './VisionModelManager';

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

  return (
    <div className="space-y-4 border-t pt-4">
      <div className="flex items-center justify-between">
        <Label className="text-sm font-medium">Participant detection</Label>
        <Switch
          checked={cfg.enabled}
          onCheckedChange={(v) => update({ enabled: v })}
          disabled={saving}
        />
      </div>

      {cfg.enabled && (
        <>
          <div>
            <Label className="text-xs text-muted-foreground">Detection mode</Label>
            <Select
              value={cfg.mode}
              onValueChange={(v) => update({ mode: v as DetectionMode })}
            >
              <SelectTrigger className="mt-1">
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
          </div>

          {cfg.mode !== 'integrated' && (
            <fieldset className="space-y-3 border rounded-md p-3">
              <legend className="px-1 text-xs font-medium text-muted-foreground">
                AI model
              </legend>
              <div className="flex items-center gap-4 text-xs">
                <label className="inline-flex items-center gap-1.5">
                  <input
                    type="radio"
                    checked={cfg.ai.source === 'local'}
                    onChange={() =>
                      persist({
                        ...cfg,
                        ai: { ...cfg.ai, source: 'local' },
                      })
                    }
                  />
                  Local
                </label>
                <label className="inline-flex items-center gap-1.5">
                  <input
                    type="radio"
                    checked={cfg.ai.source === 'external'}
                    onChange={() =>
                      persist({
                        ...cfg,
                        ai: { ...cfg.ai, source: 'external' },
                      })
                    }
                  />
                  External
                </label>
              </div>

              {cfg.ai.source === 'local' && (
                <div className="space-y-2">
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
                  <p className="text-[11px] text-amber-600">
                    ⚠ Local vision inference ships in a follow-up. You can download and
                    select models now; detection with Local still errors until the
                    inference path is wired.
                  </p>
                </div>
              )}

              {cfg.ai.source === 'external' && (
                <div className="space-y-2">
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
            </fieldset>
          )}

          {cfg.mode !== 'ai' && (
            <fieldset className="space-y-2 border rounded-md p-3">
              <legend className="px-1 text-xs font-medium text-muted-foreground">
                Integrated (Beta)
              </legend>
              <p className="text-[11px] text-muted-foreground">
                Reads Teams / Zoom / Meet app state directly. Fast and private but may break when
                those apps release a major update.
              </p>
              {statuses.map((s) => (
                <div key={s.id} className="flex items-center justify-between text-xs">
                  <div className="flex items-center gap-2">
                    <span className="capitalize font-medium">
                      {s.id === 'teams'
                        ? 'Microsoft Teams'
                        : s.id === 'zoom'
                          ? 'Zoom'
                          : 'Google Meet'}
                    </span>
                    <StatusBadge status={s.status} />
                  </div>
                </div>
              ))}
            </fieldset>
          )}
        </>
      )}
    </div>
  );
}

function StatusBadge({ status }: { status: AdapterStatusReport['status'] }) {
  let label = '';
  let color = '';
  switch (status.state) {
    case 'ready':
      label = 'Ready';
      color = 'text-green-600';
      break;
    case 'not_detected':
      label = 'Not detected';
      color = 'text-gray-500';
      break;
    case 'unsupported':
      label = status.reason;
      color = 'text-amber-600';
      break;
    case 'error':
      label = status.message;
      color = 'text-red-600';
      break;
  }
  return <span className={`${color} text-[11px]`}>{label}</span>;
}
