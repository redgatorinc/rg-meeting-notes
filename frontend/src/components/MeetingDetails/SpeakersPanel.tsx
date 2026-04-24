'use client';

import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { toast } from 'sonner';
import {
  Check,
  Pencil,
  RefreshCw,
  Users,
  X,
} from 'lucide-react';
import type {
  DiarizationModelPack,
  DiarizationModelPackInfo,
  DiarizationStatus,
  Speaker,
} from '@/types';

interface SpeakersPanelProps {
  meetingId: string;
  /** Called after diarization completes or a speaker is renamed so the
   * parent can refetch transcripts and repaint speaker prefixes. */
  onSpeakersChanged?: () => void;
}

/**
 * Small sticky panel that lists every speaker detected by diarization in
 * this meeting. Lets the user rename `Speaker 2 → Alice`, re-run
 * diarization, and switch between model packs.
 *
 * Phase 1 wires the full UX against a stub Rust engine. The real
 * sherpa-onnx engine lands in a follow-up PR and does not change this
 * component.
 */
export function SpeakersPanel({ meetingId, onSpeakersChanged }: SpeakersPanelProps) {
  const [speakers, setSpeakers] = useState<Speaker[]>([]);
  const [status, setStatus] = useState<DiarizationStatus>({ state: 'idle' });
  const [packs, setPacks] = useState<DiarizationModelPackInfo[]>([]);
  const [selectedPack, setSelectedPack] = useState<DiarizationModelPack>('default');
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renameDraft, setRenameDraft] = useState<string>('');

  const refetchSpeakers = useCallback(async () => {
    try {
      const rows = (await invoke('speakers_list', { meetingId })) as Speaker[];
      setSpeakers(rows);
    } catch (err) {
      console.error('speakers_list failed', err);
    }
  }, [meetingId]);

  // Initial load + model pack list
  useEffect(() => {
    void refetchSpeakers();
    void (async () => {
      try {
        const currentStatus = (await invoke('diarization_status', { meetingId })) as DiarizationStatus;
        setStatus(currentStatus);
      } catch (err) {
        console.error('diarization_status failed', err);
      }
    })();
    void (async () => {
      try {
        const rows = (await invoke('diarization_list_packs')) as DiarizationModelPackInfo[];
        setPacks(rows);
      } catch (err) {
        console.error('diarization_list_packs failed', err);
      }
    })();
  }, [meetingId, refetchSpeakers]);

  // Listen for diarization events emitted by the backend.
  useEffect(() => {
    let cleanupProgress: (() => void) | undefined;
    let cleanupComplete: (() => void) | undefined;
    let cleanupError: (() => void) | undefined;

    (async () => {
      cleanupProgress = await listen<{ meeting_id: string; progress: number }>(
        'diarization-progress',
        (e) => {
          if (e.payload.meeting_id === meetingId) {
            setStatus({ state: 'running', progress: e.payload.progress });
          }
        },
      );
      cleanupComplete = await listen<{ meeting_id: string; speaker_count: number }>(
        'diarization-complete',
        (e) => {
          if (e.payload.meeting_id === meetingId) {
            setStatus({ state: 'done', speaker_count: e.payload.speaker_count });
            void refetchSpeakers();
            onSpeakersChanged?.();
          }
        },
      );
      cleanupError = await listen<{ meeting_id: string; message: string }>(
        'diarization-error',
        (e) => {
          if (e.payload.meeting_id === meetingId) {
            setStatus({ state: 'error', message: e.payload.message });
            toast.error(e.payload.message);
          }
        },
      );
    })();

    return () => {
      cleanupProgress?.();
      cleanupComplete?.();
      cleanupError?.();
    };
  }, [meetingId, onSpeakersChanged, refetchSpeakers]);

  const runDiarization = useCallback(async () => {
    setStatus({ state: 'running', progress: 0 });
    try {
      await invoke('diarization_start', { meetingId, pack: selectedPack });
    } catch (err) {
      const msg = typeof err === 'string' ? err : (err as { message?: string })?.message ?? 'Diarization failed';
      toast.error(msg);
      setStatus({ state: 'error', message: msg });
    }
  }, [meetingId, selectedPack]);

  const commitRename = useCallback(
    async (speakerId: string) => {
      const name = renameDraft.trim();
      try {
        await invoke('speaker_rename', {
          speakerId,
          displayName: name.length > 0 ? name : null,
        });
        setRenamingId(null);
        setRenameDraft('');
        await refetchSpeakers();
        onSpeakersChanged?.();
      } catch (err) {
        toast.error(typeof err === 'string' ? err : 'Rename failed');
      }
    },
    [renameDraft, refetchSpeakers, onSpeakersChanged],
  );

  const formatMs = (ms: number) => {
    const totalSec = Math.round(ms / 1000);
    const m = Math.floor(totalSec / 60);
    const s = totalSec % 60;
    return `${m}:${s.toString().padStart(2, '0')}`;
  };

  const speakerLabel = (s: Speaker) =>
    s.display_name?.trim() || `Speaker ${s.cluster_idx + 1}`;

  const isBusy = status.state === 'running' || status.state === 'downloading';
  const progressPct =
    status.state === 'running' || status.state === 'downloading'
      ? Math.round(status.progress * 100)
      : null;

  return (
    <div className="sticky top-0 z-10 bg-white border-b border-gray-200 px-3 py-2 text-sm">
      <div className="flex items-center justify-between gap-2 mb-1">
        <div className="flex items-center gap-1.5 text-gray-700 font-medium">
          <Users className="w-4 h-4" />
          <span>Speakers</span>
          {speakers.length > 0 && (
            <span className="text-xs text-muted-foreground">({speakers.length})</span>
          )}
        </div>
        <div className="flex items-center gap-2">
          <select
            value={selectedPack}
            onChange={(e) => setSelectedPack(e.target.value as DiarizationModelPack)}
            disabled={isBusy}
            className="text-xs border border-gray-200 rounded px-1 py-0.5 bg-white"
            title="Model pack"
          >
            {packs.length === 0 && <option value="default">Default</option>}
            {packs.map((p) => (
              <option key={p.pack} value={p.pack}>
                {p.pack === 'default' ? 'Default' : p.pack === 'fast' ? 'Fast' : 'Accurate'}
                {' · '}
                {p.size_mb} MB
              </option>
            ))}
          </select>
          <button
            type="button"
            onClick={runDiarization}
            disabled={isBusy}
            className="text-xs text-primary hover:underline disabled:opacity-50 inline-flex items-center gap-1"
            title="Run or re-run diarization"
          >
            <RefreshCw className={`h-3 w-3 ${isBusy ? 'animate-spin' : ''}`} />
            {speakers.length === 0 ? 'Diarize' : 'Re-run'}
          </button>
          <button
            type="button"
            onClick={async () => {
              const toastId = toast.loading('Detecting participants…');
              try {
                const result = await invoke<{
                  participants: { name: string }[];
                  current_speaker?: string | null;
                  provider_host: string;
                  source_app: string;
                }>('participant_detect_snapshot', { meetingId });
                const names = result.participants.map((p) => p.name).join(', ');
                const via = result.provider_host
                  ? ` · via ${result.provider_host}`
                  : '';
                const spoken = result.current_speaker
                  ? ` · ${result.current_speaker} speaking`
                  : '';
                toast.success(
                  `Identified ${result.participants.length} participant${result.participants.length === 1 ? '' : 's'}${spoken}${via}${
                    names ? ` — ${names}` : ''
                  }`,
                  { id: toastId, duration: 5000 },
                );
                await refetchSpeakers();
                onSpeakersChanged?.();
              } catch (err) {
                toast.error(typeof err === 'string' ? err : 'Detection failed', {
                  id: toastId,
                });
              }
            }}
            className="text-xs text-primary hover:underline disabled:opacity-50 inline-flex items-center gap-1"
            title="Identify participants using the configured mode (Integrated / AI)"
          >
            📸 Detect
          </button>
        </div>
      </div>

      {progressPct !== null && (
        <div className="h-1 w-full bg-gray-100 rounded overflow-hidden mb-2">
          <div
            className="h-full bg-blue-500 transition-all"
            style={{ width: `${progressPct}%` }}
          />
        </div>
      )}

      {speakers.length === 0 && status.state === 'idle' && (
        <p className="text-xs text-muted-foreground">
          Run diarization to identify speakers in this meeting.
        </p>
      )}

      {speakers.length > 0 && (
        <ul className="space-y-1">
          {speakers.map((s) => {
            const isEditing = renamingId === s.id;
            return (
              <li
                key={s.id}
                className="flex items-center gap-2 py-0.5 text-xs"
              >
                <span
                  className="inline-block w-2 h-2 rounded-full shrink-0"
                  style={{ backgroundColor: colorForCluster(s.cluster_idx) }}
                  aria-hidden="true"
                />
                {isEditing ? (
                  <>
                    <input
                      autoFocus
                      value={renameDraft}
                      onChange={(e) => setRenameDraft(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') void commitRename(s.id);
                        if (e.key === 'Escape') {
                          setRenamingId(null);
                          setRenameDraft('');
                        }
                      }}
                      placeholder={`Speaker ${s.cluster_idx + 1}`}
                      className="flex-1 px-1.5 py-0.5 border border-gray-300 rounded text-xs"
                    />
                    <button
                      type="button"
                      onClick={() => void commitRename(s.id)}
                      className="text-green-600 hover:text-green-700"
                      title="Save"
                    >
                      <Check className="h-3.5 w-3.5" />
                    </button>
                    <button
                      type="button"
                      onClick={() => {
                        setRenamingId(null);
                        setRenameDraft('');
                      }}
                      className="text-gray-500 hover:text-gray-700"
                      title="Cancel"
                    >
                      <X className="h-3.5 w-3.5" />
                    </button>
                  </>
                ) : (
                  <>
                    <span className="flex-1 truncate">{speakerLabel(s)}</span>
                    <span className="text-muted-foreground tabular-nums">
                      {formatMs(s.total_speaking_ms)}
                    </span>
                    <button
                      type="button"
                      onClick={() => {
                        setRenamingId(s.id);
                        setRenameDraft(s.display_name ?? '');
                      }}
                      className="text-gray-400 hover:text-gray-700"
                      title="Rename"
                    >
                      <Pencil className="h-3 w-3" />
                    </button>
                  </>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}

// Stable-but-cheap HSL color per cluster index so each speaker gets a
// consistent swatch across renders and across sessions.
function colorForCluster(idx: number): string {
  const hue = (idx * 137) % 360; // golden-angle for visual separation
  return `hsl(${hue}, 70%, 50%)`;
}
