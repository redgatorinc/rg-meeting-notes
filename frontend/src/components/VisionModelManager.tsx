'use client';

import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { AnimatePresence, motion } from 'framer-motion';
import { AlertTriangle, Check, Eye, Loader2, Trash2 } from 'lucide-react';
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
  className?: string;
}

/**
 * Local vision-model registry + downloader. Mirrors the shape of
 * `QwenAsrModelManager`: backend owns discover_models + download +
 * delete, frontend listens for the three download-lifecycle events
 * and throttles per-model progress into per-row state.
 */
export function VisionModelManager({
  selectedId,
  onSelect,
  enabled = true,
  className = '',
}: Props) {
  const [models, setModels] = useState<VisionModel[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<Record<string, 'downloading' | 'deleting'>>({});
  const progressThrottle = useRef<Map<string, { progress: number; ts: number }>>(new Map());

  const reload = useCallback(async () => {
    try {
      setError(null);
      const rows = (await invoke('vision_models_list')) as VisionModel[];
      setModels(rows);
    } catch (err) {
      console.error('vision_models_list failed', err);
      setError(typeof err === 'string' ? err : 'Failed to load vision models');
    } finally {
      setLoading(false);
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

  if (loading) {
    return (
      <div className={`space-y-3 ${className}`}>
        <div className="animate-pulse space-y-3">
          <div className="h-20 rounded-lg bg-gray-100" />
          <div className="h-20 rounded-lg bg-gray-100" />
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className={`rounded-lg border border-red-200 bg-red-50 p-4 ${className}`}>
        <p className="text-sm text-red-800">Failed to load AI models</p>
        <p className="mt-1 text-xs text-red-600">{error}</p>
      </div>
    );
  }

  const selectedModel = models.find((m) => m.id === selectedId);

  return (
    <div className={`space-y-3 ${className}`}>
      {models.map((m) => {
        return (
          <VisionModelCard
            key={m.id}
            model={m}
            isSelected={selectedId === m.id && m.status.state === 'available'}
            enabled={enabled}
            isBusy={busy[m.id]}
            onSelect={() => onSelect(m.id)}
            onDownload={() => download(m.id)}
            onDelete={() => remove(m.id)}
          />
        );
      })}

      {selectedModel && selectedModel.status.state === 'available' && (
        <motion.div
          initial={{ opacity: 0, y: -5 }}
          animate={{ opacity: 1, y: 0 }}
          className="pt-2 text-center text-xs text-gray-500"
        >
          Using {selectedModel.display_name} for participant detection
        </motion.div>
      )}
    </div>
  );
}

interface VisionModelCardProps {
  model: VisionModel;
  isSelected: boolean;
  enabled: boolean;
  isBusy?: 'downloading' | 'deleting';
  onSelect: () => void;
  onDownload: () => void;
  onDelete: () => void;
}

function VisionModelCard({
  model,
  isSelected,
  enabled,
  isBusy,
  onSelect,
  onDownload,
  onDelete,
}: VisionModelCardProps) {
  const [isHovered, setIsHovered] = useState(false);
  const isAvailable = model.status.state === 'available';
  const isMissing = model.status.state === 'missing';
  const isCorrupted = model.status.state === 'corrupted';
  const isDownloading = model.status.state === 'downloading' || isBusy === 'downloading';
  const downloadProgress = model.status.state === 'downloading' ? model.status.progress : null;

  return (
    <motion.div
      initial={{ opacity: 0, y: 5 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.2 }}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      className={`
        relative rounded-lg border-2 transition-all
        ${isSelected
          ? 'border-blue-500 bg-blue-50'
          : isAvailable
            ? 'border-gray-200 bg-white hover:border-gray-300'
            : 'border-gray-200 bg-gray-50'
        }
        ${isAvailable && enabled ? 'cursor-pointer' : 'cursor-default'}
      `}
      onClick={() => {
        if (isAvailable && enabled) onSelect();
      }}
    >
      {model.id === 'moondream2' && (
        <div className="absolute -right-2 -top-2 rounded-full bg-blue-600 px-2 py-0.5 text-xs font-medium text-white">
          Recommended
        </div>
      )}

      <div className="p-4">
        <div className="mb-3 flex items-start justify-between">
          <div className="min-w-0 flex-1">
            <div className="mb-1 flex flex-wrap items-center gap-2">
              <Eye className="h-5 w-5 text-gray-500" />
              <h3 className="font-semibold text-gray-900">{model.display_name}</h3>
              {isSelected && (
                <motion.span
                  initial={{ scale: 0 }}
                  animate={{ scale: 1 }}
                  className="flex items-center gap-1 rounded-full bg-blue-600 px-2 py-0.5 text-xs font-medium text-white"
                >
                  <Check className="h-3 w-3" />
                </motion.span>
              )}
            </div>
            <p className="ml-7 text-sm text-gray-600">{model.description}</p>
            <div className="ml-7 mt-1.5 flex items-center gap-4 text-sm text-gray-600">
              <span>{formatVisionModelSize(model.size_mb)}</span>
              <span>Vision detection</span>
            </div>
          </div>

          <div className="ml-4 flex shrink-0 items-center gap-2">
            {isAvailable && (
              <>
                <div className="flex items-center gap-1.5 text-green-600">
                  <div className="h-2 w-2 rounded-full bg-green-500" />
                  <span className="text-xs font-medium">Ready</span>
                </div>
                <AnimatePresence>
                  {isHovered && (
                    <motion.button
                      type="button"
                      initial={{ opacity: 0, scale: 0.8 }}
                      animate={{ opacity: 1, scale: 1 }}
                      exit={{ opacity: 0, scale: 0.8 }}
                      transition={{ duration: 0.15 }}
                      onClick={(e) => {
                        e.stopPropagation();
                        onDelete();
                      }}
                      disabled={isBusy === 'deleting'}
                      className="p-1 text-gray-400 transition-colors hover:text-red-600 disabled:opacity-40"
                      title="Delete model to free up space"
                    >
                      {isBusy === 'deleting' ? (
                        <Loader2 className="h-4 w-4 animate-spin" />
                      ) : (
                        <Trash2 className="h-4 w-4" />
                      )}
                    </motion.button>
                  )}
                </AnimatePresence>
              </>
            )}

            {isMissing && !isDownloading && (
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  onDownload();
                }}
                className="rounded-md bg-blue-600 px-3 py-1.5 text-sm font-medium text-white transition-colors hover:bg-blue-700"
              >
                Download
              </button>
            )}

            {isDownloading && (
              <div className="flex items-center gap-2 text-blue-600">
                <Loader2 className="h-4 w-4 animate-spin" />
                <span className="text-xs font-medium">Downloading</span>
              </div>
            )}

            {isCorrupted && (
              <div className="flex items-center gap-2">
                <AlertTriangle className="h-4 w-4 text-orange-600" />
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    onDelete();
                  }}
                  className="rounded-md bg-orange-600 px-3 py-1.5 text-sm font-medium text-white transition-colors hover:bg-orange-700"
                >
                  Delete
                </button>
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    onDownload();
                  }}
                  className="rounded-md bg-blue-600 px-3 py-1.5 text-sm font-medium text-white transition-colors hover:bg-blue-700"
                >
                  Re-download
                </button>
              </div>
            )}
          </div>
        </div>

        {downloadProgress !== null && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: 'auto' }}
            className="mt-3 border-t border-gray-200 pt-3"
          >
            <div className="mb-2 flex items-center justify-between">
              <div className="flex items-center gap-2">
                <span className="text-sm font-medium text-blue-600">Downloading...</span>
                <span className="text-sm font-semibold text-blue-600">
                  {Math.round(downloadProgress)}%
                </span>
              </div>
            </div>
            <div className="h-2 w-full overflow-hidden rounded-full bg-gray-200">
              <motion.div
                className="h-full rounded-full bg-gradient-to-r from-blue-500 to-blue-600"
                initial={{ width: 0 }}
                animate={{ width: `${downloadProgress}%` }}
                transition={{ duration: 0.3, ease: 'easeOut' }}
              />
            </div>
            <p className="mt-1 text-xs text-gray-500">
              {formatVisionModelSize((model.size_mb * downloadProgress) / 100)} /{' '}
              {formatVisionModelSize(model.size_mb)}
            </p>
          </motion.div>
        )}
      </div>
    </motion.div>
  );
}

function formatVisionModelSize(sizeMb: number) {
  if (sizeMb >= 1024) {
    return `${(sizeMb / 1024).toFixed(1)} GB`;
  }

  return `${Math.round(sizeMb)} MB`;
}
