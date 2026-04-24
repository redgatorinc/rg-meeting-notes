'use client';

import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Check, Download, Loader2, Trash2 } from 'lucide-react';
import { toast } from 'sonner';

type VisionModelStatus =
  | { state: 'missing' }
  | { state: 'downloading'; progress: number }
  | { state: 'available' }
  | { state: 'corrupted'; file_size: number; expected_min_size: number };

interface VisionModel {
  id: string;
  display_name: string;
  size_mb: number;
  description: string;
  status: VisionModelStatus;
}

interface Props {
  /** Currently selected model id (from ParticipantDetectionConfig). */
  selectedId: string | null;
  /** Called when the user picks a (now-installed) model. */
  onSelect: (id: string) => void;
  /** Whether selection controls are enabled (e.g. user's AI source is local). */
  enabled?: boolean;
}

/**
 * Local vision-model registry + downloader. Mirrors the shape of
 * `QwenAsrModelManager`: backend owns discover_models + download +
 * delete, frontend listens for the three download-lifecycle events
 * and throttles per-model progress into per-row state.
 */
export function VisionModelManager({ selectedId, onSelect, enabled = true }: Props) {
  const [models, setModels] = useState<VisionModel[]>([]);
  const [busy, setBusy] = useState<Record<string, 'downloading' | 'deleting'>>({});
  const progressThrottle = useRef<Map<string, { progress: number; ts: number }>>(new Map());

  const reload = useCallback(async () => {
    try {
      const rows = (await invoke('vision_models_list')) as VisionModel[];
      setModels(rows);
    } catch (err) {
      console.error('vision_models_list failed', err);
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  useEffect(() => {
    let clP: (() => void) | undefined;
    let clC: (() => void) | undefined;
    let clE: (() => void) | undefined;
    (async () => {
      clP = await listen<{ model_id: string; progress: number }>(
        'vision-model-download-progress',
        (e) => {
          const { model_id, progress } = e.payload;
          const now = Date.now();
          const prev = progressThrottle.current.get(model_id);
          const enough =
            !prev || now - prev.ts >= 500 || Math.abs(progress - prev.progress) >= 5;
          if (!enough) return;
          progressThrottle.current.set(model_id, { progress, ts: now });
          setModels((m) =>
            m.map((row) =>
              row.id === model_id
                ? { ...row, status: { state: 'downloading', progress } }
                : row,
            ),
          );
        },
      );
      clC = await listen<{ model_id: string }>('vision-model-download-complete', (e) => {
        const id = e.payload.model_id;
        progressThrottle.current.delete(id);
        setBusy((b) => {
          const next = { ...b };
          delete next[id];
          return next;
        });
        toast.success(`Downloaded: ${id}`);
        void reload();
        // Auto-select if nothing else selected yet.
        if (!selectedId) onSelect(id);
      });
      clE = await listen<{ model_id: string; error: string }>(
        'vision-model-download-error',
        (e) => {
          const { model_id, error } = e.payload;
          progressThrottle.current.delete(model_id);
          setBusy((b) => {
            const next = { ...b };
            delete next[model_id];
            return next;
          });
          toast.error(`Download failed for ${model_id}: ${error}`);
          void reload();
        },
      );
    })();
    return () => {
      clP?.();
      clC?.();
      clE?.();
    };
  }, [reload, selectedId, onSelect]);

  const download = useCallback(async (id: string) => {
    setBusy((b) => ({ ...b, [id]: 'downloading' }));
    try {
      await invoke('vision_model_download', { modelId: id });
    } catch (err) {
      // error event will also fire; toast handled there.
      setBusy((b) => {
        const next = { ...b };
        delete next[id];
        return next;
      });
    }
  }, []);

  const remove = useCallback(async (id: string) => {
    setBusy((b) => ({ ...b, [id]: 'deleting' }));
    try {
      await invoke('vision_model_delete', { modelId: id });
      toast.success(`Deleted: ${id}`);
      void reload();
    } catch (err) {
      toast.error(typeof err === 'string' ? err : 'Delete failed');
    } finally {
      setBusy((b) => {
        const next = { ...b };
        delete next[id];
        return next;
      });
    }
  }, [reload]);

  return (
    <div className="space-y-2">
      {models.map((m) => {
        const isSelected = selectedId === m.id && m.status.state === 'available';
        const isDownloading =
          m.status.state === 'downloading' || busy[m.id] === 'downloading';
        const isAvailable = m.status.state === 'available';
        const downloadProgress =
          m.status.state === 'downloading' ? m.status.progress : null;

        return (
          <div
            key={m.id}
            className={`flex items-start gap-3 rounded-md border px-3 py-2 ${
              isSelected ? 'border-blue-400 bg-blue-50/50' : 'border-gray-200'
            }`}
          >
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-2">
                <span className="text-sm font-medium">{m.display_name}</span>
                <span className="text-[11px] text-muted-foreground">
                  {m.size_mb} MB
                </span>
                {isSelected && (
                  <span className="text-[11px] text-blue-600 inline-flex items-center gap-0.5">
                    <Check className="h-3 w-3" /> Selected
                  </span>
                )}
              </div>
              <p className="text-[11px] text-muted-foreground mt-0.5">{m.description}</p>
              {downloadProgress !== null && (
                <div className="mt-1.5 h-1 w-full bg-gray-100 rounded overflow-hidden">
                  <div
                    className="h-full bg-blue-500 transition-all"
                    style={{ width: `${downloadProgress}%` }}
                  />
                </div>
              )}
            </div>
            <div className="flex items-center gap-1 shrink-0">
              {isAvailable && (
                <button
                  type="button"
                  onClick={() => onSelect(m.id)}
                  disabled={!enabled || isSelected}
                  className="text-[11px] px-2 py-1 rounded text-primary hover:bg-gray-100 disabled:opacity-40"
                >
                  Select
                </button>
              )}
              {isAvailable ? (
                <button
                  type="button"
                  onClick={() => remove(m.id)}
                  disabled={busy[m.id] === 'deleting'}
                  className="text-[11px] px-2 py-1 rounded text-red-600 hover:bg-red-50 inline-flex items-center gap-1"
                  title="Delete model files"
                >
                  <Trash2 className="h-3 w-3" />
                </button>
              ) : (
                <button
                  type="button"
                  onClick={() => download(m.id)}
                  disabled={isDownloading}
                  className="text-[11px] px-2 py-1 rounded text-primary hover:bg-gray-100 inline-flex items-center gap-1"
                >
                  {isDownloading ? (
                    <>
                      <Loader2 className="h-3 w-3 animate-spin" />
                      {downloadProgress !== null ? `${downloadProgress}%` : '…'}
                    </>
                  ) : (
                    <>
                      <Download className="h-3 w-3" /> Download
                    </>
                  )}
                </button>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}
