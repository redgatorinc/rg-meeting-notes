'use client';

import { useEffect, useState, type ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Mic2, Radio, Video } from 'lucide-react';
import type { SelectedDevices } from './DeviceSelection';
import { useMicMuteState } from '@/hooks/useMicMuteState';
import { RecordingAudioSourcesPanel } from './RecordingAudioSourcesPanel';

interface MeetingDetectionSnapshot {
  enabled: boolean;
  active_apps: string[];
}

interface AudioSignalStatus {
  rms_level: number;
  peak_level: number;
  is_active: boolean;
  updated_ago_ms: number;
}

interface RecordingAudioStatus {
  is_recording: boolean;
  microphone_device: string | null;
  system_device: string | null;
  microphone_signal: AudioSignalStatus | null;
  system_signal: AudioSignalStatus | null;
}

interface RecordingMeetingAppStatusProps {
  isRecording: boolean;
  selectedDevices: SelectedDevices;
}

const REFRESH_INTERVAL_MS = 5000;

/**
 * Always-visible home status card for recording state, microphone status
 * (OS mute + live signal), and meeting-app detection. The per-app audio
 * source list is rendered by `RecordingAudioSourcesPanel` below this card.
 */
export function RecordingMeetingAppStatus({
  isRecording,
}: RecordingMeetingAppStatusProps) {
  const [recordingAudio, setRecordingAudio] = useState<RecordingAudioStatus>({
    is_recording: false,
    microphone_device: null,
    system_device: null,
    microphone_signal: null,
    system_signal: null,
  });
  const [meetingDetection, setMeetingDetection] =
    useState<MeetingDetectionSnapshot>({
      enabled: true,
      active_apps: [],
    });

  useEffect(() => {
    let disposed = false;
    const cleanups: Array<() => void> = [];

    const setIfMounted = <T,>(setter: (value: T) => void, value: T) => {
      if (!disposed) setter(value);
    };

    const refreshRecordingAudio = async () => {
      try {
        const snapshot = await invoke<RecordingAudioStatus>(
          'get_recording_audio_status'
        );
        setIfMounted(setRecordingAudio, snapshot);
      } catch (err) {
        console.debug('Unable to read recording audio status:', err);
      }
    };

    const refreshMeetingDetection = async () => {
      try {
        const snapshot = await invoke<MeetingDetectionSnapshot>(
          'current_meeting_detection'
        );
        setIfMounted(setMeetingDetection, {
          enabled: snapshot.enabled,
          active_apps: normalizeApps(snapshot.active_apps),
        });
      } catch (err) {
        console.debug('Unable to read meeting detection:', err);
      }
    };

    const refresh = () => {
      void refreshRecordingAudio();
      void refreshMeetingDetection();
    };

    const addListener = async <T,>(
      eventName: string,
      handler: (payload: T) => void
    ) => {
      const unlisten = await listen<T>(eventName, (event) => {
        handler(event.payload);
      });
      if (disposed) {
        unlisten();
      } else {
        cleanups.push(unlisten);
      }
    };

    void addListener<MeetingDetectionSnapshot>(
      'meeting-detection-updated',
      (snapshot) => {
        setMeetingDetection({
          enabled: snapshot.enabled,
          active_apps: normalizeApps(snapshot.active_apps),
        });
      }
    );

    refresh();
    const intervalId = window.setInterval(refresh, REFRESH_INTERVAL_MS);

    return () => {
      disposed = true;
      window.clearInterval(intervalId);
      cleanups.forEach((cleanup) => cleanup());
    };
  }, []);

  const statusLabel = isRecording ? 'Recording' : 'Not Recording';
  const micMuted = useMicMuteState(isRecording);
  const microphoneLabel = formatMicrophoneStatus(
    isRecording,
    recordingAudio.microphone_signal,
    micMuted
  );
  const meetingLabel = meetingDetection.enabled
    ? meetingDetection.active_apps.length > 0
      ? meetingDetection.active_apps.join(', ')
      : 'No meeting'
    : 'Disabled in General Settings';

  return (
    <>
      <div className="fixed top-20 right-4 z-40 w-80 max-w-[90vw] overflow-hidden rounded-lg border border-gray-200 bg-white text-xs shadow-lg">
        <div className="flex items-center border-b border-gray-100 px-3 py-2">
          <div className="flex items-center gap-1.5 text-sm font-semibold text-gray-900">
            <Radio
              className={`h-4 w-4 ${
                isRecording ? 'animate-pulse text-red-500' : 'text-gray-400'
              }`}
            />
            <span>Recording session</span>
          </div>
        </div>

        <div className="divide-y divide-gray-100">
          <StatusRow icon={<Radio className="h-3.5 w-3.5" />} label="Status">
            <StatusBadge recording={isRecording}>{statusLabel}</StatusBadge>
          </StatusRow>
          <StatusRow icon={<Mic2 className="h-3.5 w-3.5" />} label="Microphone">
            {microphoneLabel}
          </StatusRow>
          <StatusRow icon={<Video className="h-3.5 w-3.5" />} label="Meeting Detection">
            {meetingLabel}
          </StatusRow>
        </div>
      </div>
      <RecordingAudioSourcesPanel isRecording={isRecording} />
    </>
  );
}

function StatusRow({
  icon,
  label,
  children,
}: {
  icon: ReactNode;
  label: string;
  children: ReactNode;
}) {
  return (
    <div className="flex items-start gap-2 px-3 py-2">
      <div className="mt-0.5 text-gray-400">{icon}</div>
      <div className="min-w-0 flex-1">
        <div className="text-[11px] font-medium uppercase tracking-wide text-gray-500">
          {label}
        </div>
        <div className="mt-0.5 break-words font-medium leading-snug text-gray-900">
          {children}
        </div>
      </div>
    </div>
  );
}

function StatusBadge({
  recording,
  children,
}: {
  recording: boolean;
  children: ReactNode;
}) {
  const className = recording
    ? 'border-red-200 bg-red-50 text-red-700'
    : 'border-gray-200 bg-gray-50 text-gray-600';

  return (
    <span
      className={`inline-flex items-center rounded-full border px-2 py-0.5 text-[11px] font-medium ${className}`}
    >
      {children}
    </span>
  );
}

function normalizeApps(apps: string[] | null | undefined): string[] {
  const seen = new Set<string>();
  const normalized: string[] = [];

  for (const app of apps ?? []) {
    const name = app.trim();
    if (!name || seen.has(name.toLowerCase())) continue;
    seen.add(name.toLowerCase());
    normalized.push(name);
  }

  return normalized.sort((a, b) => a.localeCompare(b));
}

function formatMicrophoneStatus(
  isRecording: boolean,
  signal: AudioSignalStatus | null,
  osMuted: boolean | null
) {
  if (!isRecording) return 'Not recording';
  if (osMuted === true) return 'Muted';
  if (!signal || signal.updated_ago_ms > 3000) return 'Waiting for input signal';
  if (signal.is_active) return 'Input signal active';
  return osMuted === false ? 'Silent' : 'Silent / muted';
}
