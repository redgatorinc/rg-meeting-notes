'use client';

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { Download, Trash2, CheckCircle2, Loader2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { formatBytes } from '@/lib/formatting';

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

  const refresh = async () => {
    try {
      const list = await invoke<DiarizationModelInfo[]>('diarization_models_list');
      setModels(list ?? []);
    } catch (err) {
      console.error('diarization_models_list failed:', err);
    }
  };

  useEffect(() => {
    void refresh();
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
      void refresh();
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

        return (
          <div key={m.id} className="flex items-start gap-3 px-4 py-3">
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <div className="truncate text-sm font-medium text-gray-900">
                  {m.display_name}
                </div>
                {m.installed && !isDownloading && (
                  <CheckCircle2 className="h-4 w-4 flex-shrink-0 text-green-600" />
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
                  disabled={isDownloading}
                  title="Delete this pack"
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
