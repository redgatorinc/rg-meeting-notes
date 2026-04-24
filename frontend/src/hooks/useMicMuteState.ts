'use client';

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

const POLL_INTERVAL_MS = 1000;

/**
 * Polls the OS-level default capture endpoint mute state while `isRecording`.
 * Returns `null` on unsupported platforms (mac/Linux) or on transient errors —
 * consumers should fall back to activity-based heuristics in that case.
 */
export function useMicMuteState(isRecording: boolean): boolean | null {
  const [muted, setMuted] = useState<boolean | null>(null);

  useEffect(() => {
    if (!isRecording) {
      setMuted(null);
      return;
    }

    let disposed = false;

    const poll = async () => {
      try {
        const result = await invoke<boolean | null>('get_microphone_mute_state');
        if (!disposed) setMuted(result);
      } catch {
        if (!disposed) setMuted(null);
      }
    };

    void poll();
    const id = window.setInterval(() => void poll(), POLL_INTERVAL_MS);
    return () => {
      disposed = true;
      window.clearInterval(id);
    };
  }, [isRecording]);

  return muted;
}
