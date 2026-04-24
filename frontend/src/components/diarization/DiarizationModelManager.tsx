'use client';

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { Download, Trash2, CheckCircle2, Loader2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { formatBytes } from '@/lib/formatting';

const STORE_FILE = 'preferences.json';
const KEY_MODEL_PACK = 'diarization.model_pack';

interface DiarizationModelInfo {
  id: string;            // 'default' | 'fast' | 'accurate'
  display_name: string;
  description: string;
  size_mb: number;
  installed: boolean;
}

interface DiarizationDownloadProgress {
  pack_id: string;
  stage: string;         // 'segmentation' | 'embedding'
  downloaded_bytes: number;
  total_bytes: number;
  percent: number;
}

type PackId = 'default' | 'fast' | 'accurate';

/**
 * Settings-page component that lists the three diarization packs, shows
 * install state, and lets the user download / delete each. Mirrors the
 * VisionModelManager / QwenAsrModelManager shape verbatim.
 */
export function DiarizationModelManager() {
  const [models, setModels] = useState<DiarizationModelInfo[]>([]);
  const [progress, setProgress] = useState<Record<string, DiarizationDownloadProgress>>({});
  const [busy, setBusy] = useState<Record<string, boolean>>({});
  const [selectedPack, setSelectedPack] = useState<string>('default');

  const refresh = async () => {
    try {
      const list = await invoke<DiarizationModelInfo[]>('diarization_models_list');
      setModels(list ?? []);
    } catch (err) {
      console.error('diarization_models_list failed:', err);
    }
  };

  const loadSelected = async () => {
    try {
      const { Store } = await import('@tauri-apps/plugin-store');
      const store = await Store.load(STORE_FILE);
      const saved = (await store.get<string>(KEY_MODEL_PACK)) ?? 'default';
      setSelectedPack(saved);
    } catch (err) {
      console.warn('Failed to read diarization.model_pack:', err);
    }
  };

  const persistSelected = async (packId: string) => {
    setSelectedPack(packId);
    try {
      const { Store } = await import('@tauri-apps/plugin-store');
      const store = await Store.load(STORE_FILE);
      await store.set(KEY_MODEL_PACK, packId);
      await store.save();
    } catch (err) {
      console.error('Failed to persist diarization.model_pack:', err);
    }
  };

  useEffect(() => {
    void refresh();
    void loadSelected();
    const unlisteners: UnlistenFn[] = [];

    void listen<DiarizationDownloadProgress>(
      'diarization-model-download-progress',
      (e) => setProgress((prev) => ({ ...prev, [e.payload.pack_id]: e.payload })),
    ).then((u) => unlisteners.push(u));

    void listen<{ pack_id: string }>('diarization-model-download-complete', (e) => {
      setProgress((prev) => {
        const next = { ...prev };
        delete next[e.payload.pack_id];
        return next;
      });
      setBusy((prev) => ({ ...prev, [e.payload.pack_id]: false }));
      void (async () => {
        await refresh();
        // If nothing is currently installed-and-selected, auto-select the
        // pack that just finished downloading so the user doesn't have to
        // hunt for the radio.
        const installedNow = (await invoke<DiarizationModelInfo[]>('diarization_models_list')).filter(
          (m) => m.installed,
        );
        const activeInstalled = installedNow.some((m) => m.id === selectedPack);
        if (!activeInstalled) {
          await persistSelected(e.payload.pack_id);
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
      setBusy((prev) => ({ ...prev, [e.payload.pack_id]: false }));
      void refresh();
    }).then((u) => unlisteners.push(u));

    return () => {
      unlisteners.forEach((u) => u());
    };
  }, []);

  const handleDownload = async (packId: string) => {
    setBusy((prev) => ({ ...prev, [packId]: true }));
    try {
      await invoke('diarization_model_download', { pack: packId as PackId });
    } catch (err) {
      console.error('diarization_model_download failed:', err);
      setBusy((prev) => ({ ...prev, [packId]: false }));
    }
  };

  const handleDelete = async (packId: string) => {
    try {
      await invoke('diarization_model_delete', { pack: packId as PackId });
      await refresh();
    } catch (err) {
      console.error('diarization_model_delete failed:', err);
    }
  };

  return (
    <div className="divide-y divide-gray-100 rounded-lg border border-gray-200 bg-white">
      {models.map((m) => {
        const dl = progress[m.id];
        const isDownloading = !!dl || !!busy[m.id];
        const pct = dl?.percent ?? 0;
        const isActive = selectedPack === m.id && m.installed;

        return (
          <div key={m.id} className="flex items-start gap-3 px-4 py-3">
            {/* Select radio — only clickable when the pack is installed */}
            <label
              className={`mt-1 flex-shrink-0 ${
                m.installed ? 'cursor-pointer' : 'cursor-not-allowed opacity-40'
              }`}
              title={m.installed ? 'Use this pack for diarization' : 'Download to enable'}
            >
              <input
                type="radio"
                name="diarization-pack"
                checked={selectedPack === m.id}
                disabled={!m.installed}
                onChange={() => void persistSelected(m.id)}
                className="h-4 w-4 cursor-pointer accent-blue-600"
              />
            </label>

            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <div className="truncate text-sm font-medium text-gray-900">
                  {m.display_name}
                </div>
                {m.installed && !isDownloading && (
                  <CheckCircle2 className="h-4 w-4 flex-shrink-0 text-green-600" />
                )}
                {isActive && (
                  <span className="inline-flex items-center rounded-full bg-blue-50 px-2 py-0.5 text-[10px] font-semibold text-blue-700">
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

            <div className="flex flex-shrink-0 gap-1">
              {m.installed ? (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => void handleDelete(m.id)}
                  disabled={isDownloading || isActive}
                  title={isActive ? 'Select another pack first' : 'Delete this pack'}
                  className="text-red-600 hover:text-red-700"
                >
                  <Trash2 className="h-4 w-4" />
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
        );
      })}
    </div>
  );
}
