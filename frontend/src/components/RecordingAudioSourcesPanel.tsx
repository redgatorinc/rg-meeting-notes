'use client';

import { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Mic2, Monitor, AppWindow } from 'lucide-react';
import { useAudioSources } from '@/hooks/useAudioSources';
import { useUserDisplayName } from '@/hooks/useUserDisplayName';
import type { AudioSource } from '@/types';

interface RecordingAudioSourcesPanelProps {
  isRecording: boolean;
}

interface AudioSignalStatus {
  rms_level: number;
  peak_level: number;
  is_active: boolean;
  updated_ago_ms: number;
}

interface RecordingAudioStatus {
  microphone_signal: AudioSignalStatus | null;
}

const SIGNAL_POLL_MS = 500;

/**
 * Live list of audio sources currently contributing to the recording.
 * On Windows this reflects WASAPI session enumeration (Microphone + any app
 * producing audio). On macOS / Linux the backend returns a trimmed stub
 * (Microphone + System audio).
 */
export function RecordingAudioSourcesPanel({ isRecording }: RecordingAudioSourcesPanelProps) {
  const sources = useAudioSources(isRecording);
  const userName = useUserDisplayName();
  const micLive = useLiveMicActive(isRecording);

  // Override the mic row's `active` with the live capture-stream signal —
  // the Rust side can only see the OS mute flag, not real-time speech.
  const rows = useMemo<AudioSource[]>(
    () =>
      sources.map((s) =>
        s.kind === 'microphone' ? { ...s, active: micLive } : s
      ),
    [sources, micLive]
  );

  if (!isRecording) return null;

  return (
    <div className="fixed top-[18rem] right-4 z-40 w-80 max-w-[90vw] overflow-hidden rounded-lg border border-gray-200 bg-white text-xs shadow-lg">
      <div className="border-b border-gray-100 px-3 py-2">
        <div className="text-[11px] font-medium uppercase tracking-wide text-gray-500">
          Recording audio from
        </div>
      </div>
      <div className="divide-y divide-gray-100">
        {rows.length === 0 ? (
          <div className="px-3 py-3 text-gray-500">Listening for audio sources…</div>
        ) : (
          rows.map((source) => <AudioSourceRow key={source.id} source={source} userName={userName} />)
        )}
      </div>
    </div>
  );
}

function useLiveMicActive(isRecording: boolean): boolean {
  const [active, setActive] = useState(false);

  useEffect(() => {
    if (!isRecording) {
      setActive(false);
      return;
    }
    let disposed = false;
    const poll = async () => {
      try {
        const snap = await invoke<RecordingAudioStatus>('get_recording_audio_status');
        if (disposed) return;
        const sig = snap.microphone_signal;
        // Fresh + above-noise-floor — same semantics as the status card.
        const isLive = !!sig && sig.updated_ago_ms <= 3000 && sig.is_active;
        setActive(isLive);
      } catch {
        if (!disposed) setActive(false);
      }
    };
    void poll();
    const id = window.setInterval(() => void poll(), SIGNAL_POLL_MS);
    return () => {
      disposed = true;
      window.clearInterval(id);
    };
  }, [isRecording]);

  return active;
}

function AudioSourceRow({ source, userName }: { source: AudioSource; userName: string }) {
  const icon =
    source.kind === 'microphone' ? (
      <Mic2 className="h-3.5 w-3.5" />
    ) : source.kind === 'system' ? (
      <Monitor className="h-3.5 w-3.5" />
    ) : (
      <AppWindow className="h-3.5 w-3.5" />
    );

  const dotClass = source.active
    ? 'bg-green-500'
    : 'bg-gray-300';

  const label =
    source.kind === 'microphone'
      ? userName
        ? `${source.display_name} (${userName})`
        : `${source.display_name} (you)`
      : source.display_name;

  return (
    <div className="flex items-center gap-2 px-3 py-2">
      <span className={`inline-block h-2 w-2 flex-shrink-0 rounded-full ${dotClass}`} aria-hidden />
      <span className="text-gray-400">{icon}</span>
      <span className="min-w-0 flex-1 truncate font-medium text-gray-900" title={label}>
        {label}
      </span>
    </div>
  );
}
