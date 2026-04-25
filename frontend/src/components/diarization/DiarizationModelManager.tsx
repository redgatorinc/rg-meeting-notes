'use client';

import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { Check, Download, Loader2, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { formatBytes } from '@/lib/formatting';

const STORE_FILE = 'preferences.json';
const KEY_MODEL_PACK = 'diarization.model_pack';

interface DiarizationModelInfo {
  id: string; // 'default' | 'fast' | 'accurate'
  display_name: string;
  description: string;
  size_mb: number;
  installed: boolean;
}

interface DiarizationDownloadProgress {
  pack_id: string;
  stage: string; // 'segmentation' | 'embedding'
  downloaded_bytes: number;
  total_bytes: number;
  percent: number;
}

type PackId = 'default' | 'fast' | 'accurate';

/**
 * Card-style list of diarization packs. Matches the VisionModelManager /
 * QwenAsrModelManager UX: click the card to select (only when installed),
 * Download / Delete buttons on the right. Active pack has a blue border
 * and an "Active" pill.
 */
export function DiarizationModelManager() {
  const [models, setModels] = useState<DiarizationModelInfo[]>([]);
  const [progress, setProgress] = useState<Record<string, DiarizationDownloadProgress>>({});
  const [busy, setBusy] = useState<Record<string, 'downloading' | 'deleting'>>({});
  const [selectedPack, setSelectedPack] = useState<string>('default');
  const selectedPackRef = useRef<string>('default');

  const refresh = useCallback(async () => {
    try {
      const list = await invoke<DiarizationModelInfo[]>('diarization_models_list');
      setModels(list ?? []);
    } catch (err) {
      console.error('diarization_models_list failed:', err);
    }
  }, []);

  const persistSelected = useCallback(async (packId: string) => {
    setSelectedPack(packId);
    selectedPackRef.current = packId;
    try {
      const { Store } = await import('@tauri-apps/plugin-store');
      const store = await Store.load(STORE_FILE);
      await store.set(KEY_MODEL_PACK, packId);
      await store.save();
    } catch (err) {
      console.error('Failed to persist diarization.model_pack:', err);
    }
  }, []);

  useEffect(() => {
    void refresh();
    (async () => {
      try {
        const { Store } = await import('@tauri-apps/plugin-store');
        const store = await Store.load(STORE_FILE);
        const saved = (await store.get<string>(KEY_MODEL_PACK)) ?? 'default';
        setSelectedPack(saved);
        selectedPackRef.current = saved;
      } catch {
        /* ignore */
      }
    })();

    const unlisteners: UnlistenFn[] = [];

    void listen<DiarizationDownloadProgress>(
      'diarization-model-download-progress',
      (e) => setProgress((prev) => ({ ...prev, [e.payload.pack_id]: e.payload })),
    ).then((u) => unlisteners.push(u));

    void listen<{ pack_id: string }>('diarization-model-download-complete', (e) => {
      const packId = e.payload.pack_id;
      setProgress((prev) => {
        const next = { ...prev };
        delete next[packId];
        return next;
      });
      setBusy((prev) => {
        const next = { ...prev };
        delete next[packId];
        return next;
      });
      void (async () => {
        await refresh();
        // Auto-select the just-downloaded pack if nothing installed is
        // currently selected — saves the user one click.
        const latest = await invoke<DiarizationModelInfo[]>('diarization_models_list');
        const installed = latest.filter((m) => m.installed);
        const activeStillInstalled = installed.some((m) => m.id === selectedPackRef.current);
        if (!activeStillInstalled) {
          await persistSelected(packId);
        }
      })();
    }).then((u) => unlisteners.push(u));

    void listen<{ pack_id: string; error: string }>('diarization-model-download-error', (e) => {
      console.error('diarization download error', e.payload);
      setProgress((prev) => {
        const next = { ...prev };
        delete next[e.payload.pack_id];
        return next;
      });
      setBusy((prev) => {
        const next = { ...prev };
        delete next[e.payload.pack_id];
        return next;
      });
      void refresh();
    }).then((u) => unlisteners.push(u));

    // Safety net — poll every 10 s while mounted so a missed
    // download-complete event (e.g. killed listener across HMR) still
    // reconciles the UI state with disk.
    const intervalId = window.setInterval(() => void refresh(), 10_000);

    return () => {
      unlisteners.forEach((u) => u());
      window.clearInterval(intervalId);
    };
  }, [refresh, persistSelected]);

  const handleDownload = async (packId: string) => {
    setBusy((b) => ({ ...b, [packId]: 'downloading' }));
    try {
      await invoke('diarization_model_download', { pack: packId as PackId });
    } catch (err) {
      console.error('diarization_model_download failed:', err);
      setBusy((b) => {
        const next = { ...b };
        delete next[packId];
        return next;
      });
    }
  };

  const handleDelete = async (packId: string) => {
    setBusy((b) => ({ ...b, [packId]: 'deleting' }));
    try {
      await invoke('diarization_model_delete', { pack: packId as PackId });
      await refresh();
    } catch (err) {
      console.error('diarization_model_delete failed:', err);
    } finally {
      setBusy((b) => {
        const next = { ...b };
        delete next[packId];
        return next;
      });
    }
  };

  return (
    <div className="space-y-3">
      {models.map((m) => {
        const dl = progress[m.id];
        const state = busy[m.id];
        const isDownloading = !!dl || state === 'downloading';
        const isDeleting = state === 'deleting';
        const pct = dl?.percent ?? 0;
        const isSelected = selectedPack === m.id && m.installed;
        const clickable = m.installed && !isDownloading && !isDeleting;

        return (
          <div
            key={m.id}
            role={clickable ? 'button' : undefined}
            tabIndex={clickable ? 0 : -1}
            onClick={() => {
              if (clickable) void persistSelected(m.id);
            }}
            onKeyDown={(e) => {
              if (clickable && (e.key === 'Enter' || e.key === ' ')) {
                e.preventDefault();
                void persistSelected(m.id);
              }
            }}
            className={`
              relative rounded-lg border-2 px-4 py-3 transition-all
              ${isSelected
                ? 'border-blue-500 bg-blue-50'
                : m.installed
                  ? 'border-gray-200 bg-white hover:border-gray-300'
                  : 'border-gray-200 bg-gray-50'}
              ${clickable ? 'cursor-pointer' : 'cursor-default'}
            `}
          >
            <div className="flex items-start gap-3">
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2">
                  <div className="truncate text-sm font-medium text-gray-900">
                    {m.display_name}
                  </div>
                  {m.installed && !isDownloading && (
                    <Check className="h-4 w-4 flex-shrink-0 text-green-600" />
                  )}
                  {isSelected && (
                    <span className="inline-flex items-center rounded-full bg-blue-100 px-2 py-0.5 text-[10px] font-semibold text-blue-700">
                      Active
                    </span>
                  )}
                </div>
                <p className="mt-0.5 text-xs text-gray-500">{m.description}</p>
                <div className="mt-1 text-[11px] text-gray-500">
                  {formatBytes(m.size_mb * 1024 * 1024)}
                  {isDownloading && dl && (
                    <span className="ml-2">
                      · {dl.stage}: {pct}%
                    </span>
                  )}
                </div>
                {isDownloading && (
                  <div className="mt-2 h-1 w-full overflow-hidden rounded-full bg-gray-100">
                    <div
                      className="h-full bg-blue-500 transition-all"
                      style={{ width: `${pct}%` }}
                    />
                  </div>
                )}
              </div>

              <div className="flex flex-shrink-0 gap-1" onClick={(e) => e.stopPropagation()}>
                {m.installed ? (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => void handleDelete(m.id)}
                    disabled={isDownloading || isDeleting || isSelected}
                    title={isSelected ? 'Select another pack first' : 'Delete this pack'}
                    className="text-red-600 hover:text-red-700"
                  >
                    {isDeleting ? <Loader2 className="h-4 w-4 animate-spin" /> : <Trash2 className="h-4 w-4" />}
                  </Button>
                ) : (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => void handleDownload(m.id)}
                    disabled={isDownloading}
                  >
                    {isDownloading ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : (
                      <>
                        <Download className="mr-1.5 h-4 w-4" />
                        Download
                      </>
                    )}
                  </Button>
                )}
              </div>
            </div>
          </div>
        );
      })}

      {models.some((m) => m.installed && selectedPack === m.id) && (
        <p className="pt-1 text-center text-xs text-gray-500">
          Click a pack to make it Active for diarization.
        </p>
      )}
    </div>
  );
}
