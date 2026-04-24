'use client';

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { DiarizationStatus } from '@/types';

/**
 * Tracks the diarization state for a specific meeting. Combines an initial
 * poll (via `diarization_status`) with live updates from the three Tauri
 * events the backend emits:
 *   - `diarization-progress`
 *   - `diarization-complete`
 *   - `diarization-error`
 * Each event is meeting-scoped; we filter by `meeting_id`.
 */
export function useDiarizationStatus(meetingId: string | null): DiarizationStatus {
  const [status, setStatus] = useState<DiarizationStatus>({ state: 'idle' });

  useEffect(() => {
    if (!meetingId) {
      setStatus({ state: 'idle' });
      return;
    }

    let disposed = false;
    const unlisteners: UnlistenFn[] = [];

    const refresh = async () => {
      try {
        const s = await invoke<DiarizationStatus>('diarization_status', { meetingId });
        if (!disposed && s) setStatus(s);
      } catch {
        if (!disposed) setStatus({ state: 'idle' });
      }
    };

    const applyIfMatch = (payload: { meeting_id?: string } & Record<string, unknown>) =>
      !payload.meeting_id || payload.meeting_id === meetingId;

    void listen<{ meeting_id: string; progress: number }>('diarization-progress', (e) => {
      if (applyIfMatch(e.payload)) {
        setStatus({ state: 'running', progress: e.payload.progress });
      }
    }).then((u) => unlisteners.push(u));

    void listen<{ meeting_id: string; speaker_count: number }>('diarization-complete', (e) => {
      if (applyIfMatch(e.payload)) {
        setStatus({ state: 'done', speaker_count: e.payload.speaker_count });
      }
    }).then((u) => unlisteners.push(u));

    void listen<{ meeting_id: string; message: string }>('diarization-error', (e) => {
      if (applyIfMatch(e.payload)) {
        setStatus({ state: 'error', message: e.payload.message });
      }
    }).then((u) => unlisteners.push(u));

    void refresh();
    return () => {
      disposed = true;
      unlisteners.forEach((u) => u());
    };
  }, [meetingId]);

  return status;
}
