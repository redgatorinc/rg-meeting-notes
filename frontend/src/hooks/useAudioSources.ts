'use client';

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { AudioSource } from '@/types';

const POLL_INTERVAL_MS = 2000;

/**
 * Polls the backend for the current list of audio sources — microphone plus
 * any apps producing audio through WASAPI sessions (Windows only; mac/Linux
 * return a trimmed stub).
 */
export function useAudioSources(isRecording: boolean): AudioSource[] {
  const [sources, setSources] = useState<AudioSource[]>([]);

  useEffect(() => {
    if (!isRecording) {
      setSources([]);
      return;
    }

    let disposed = false;

    const poll = async () => {
      try {
        const result = await invoke<AudioSource[]>('list_audio_sources');
        if (!disposed) setSources(result ?? []);
      } catch {
        if (!disposed) setSources([]);
      }
    };

    void poll();
    const id = window.setInterval(() => void poll(), POLL_INTERVAL_MS);
    return () => {
      disposed = true;
      window.clearInterval(id);
    };
  }, [isRecording]);

  return sources;
}
