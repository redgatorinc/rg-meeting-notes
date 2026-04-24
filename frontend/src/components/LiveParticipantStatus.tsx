'use client';

import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Eye, Loader2, RefreshCw, X } from 'lucide-react';

type DetectionState =
  | { kind: 'idle' }
  | { kind: 'detecting' }
  | {
      kind: 'done';
      participants: string[];
      currentSpeaker: string | null;
      providerHost: string;
      sourceApp: string;
      at: number;
    }
  | { kind: 'error'; message: string; at: number };

interface LiveParticipantStatusProps {
  /** Show only when recording is active. */
  visible: boolean;
  /** Poll cadence. `null` disables auto-trigger. */
  autoIntervalMs?: number | null;
}

/**
 * Floating participant-detection status card on Home.
 *
 * Shown while recording. Provides a manual `Detect now` button and
 * (optionally) auto-triggers every N seconds to refresh the roster.
 * Calls `participant_detect_snapshot` with no meeting_id so the
 * backend does not try to auto-rename any DB row during the live
 * session — the result is rendered in-place only.
 */
export function LiveParticipantStatus({
  visible,
  autoIntervalMs = null,
}: LiveParticipantStatusProps) {
  const [state, setState] = useState<DetectionState>({ kind: 'idle' });
  const [dismissed, setDismissed] = useState(false);
  const inFlight = useRef(false);

  const runDetection = useCallback(async () => {
    if (inFlight.current) return;
    inFlight.current = true;
    setState({ kind: 'detecting' });
    try {
      const result = await invoke<{
        participants: { name: string }[];
        current_speaker?: string | null;
        provider_host: string;
        source_app: string;
      }>('participant_detect_snapshot', { meetingId: null });
      setState({
        kind: 'done',
        participants: result.participants.map((p) => p.name),
        currentSpeaker: result.current_speaker ?? null,
        providerHost: result.provider_host,
        sourceApp: result.source_app,
        at: Date.now(),
      });
    } catch (err) {
      setState({
        kind: 'error',
        message: typeof err === 'string' ? err : 'Detection failed',
        at: Date.now(),
      });
    } finally {
      inFlight.current = false;
    }
  }, []);

  useEffect(() => {
    if (!visible || !autoIntervalMs) return;
    const id = window.setInterval(() => {
      void runDetection();
    }, autoIntervalMs);
    return () => window.clearInterval(id);
  }, [visible, autoIntervalMs, runDetection]);

  // Reset the dismiss flag when a new recording starts.
  useEffect(() => {
    if (visible) setDismissed(false);
  }, [visible]);

  if (!visible || dismissed) return null;

  return (
    <div className="fixed bottom-28 right-4 z-40 w-80 max-w-[90vw] rounded-lg border bg-white shadow-lg">
      <div className="flex items-center justify-between border-b px-3 py-2">
        <div className="flex items-center gap-1.5 text-sm font-medium">
          <Eye className="h-4 w-4 text-gray-500" />
          <span>Participants</span>
        </div>
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={runDetection}
            disabled={state.kind === 'detecting'}
            title="Detect now"
            className="inline-flex items-center justify-center w-6 h-6 rounded text-gray-600 hover:bg-gray-100 disabled:opacity-50"
          >
            {state.kind === 'detecting' ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <RefreshCw className="h-3.5 w-3.5" />
            )}
          </button>
          <button
            type="button"
            onClick={() => setDismissed(true)}
            title="Hide"
            className="inline-flex items-center justify-center w-6 h-6 rounded text-gray-500 hover:bg-gray-100"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
      </div>

      <div className="px-3 py-2 text-xs">
        {state.kind === 'idle' && (
          <p className="text-muted-foreground">
            Click the refresh button to identify participants from the active
            meeting window.
          </p>
        )}

        {state.kind === 'detecting' && (
          <div className="flex items-center gap-2 text-gray-700">
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
            <span>Detecting participants…</span>
          </div>
        )}

        {state.kind === 'error' && (
          <div className="text-red-600">
            <div className="font-medium mb-0.5">Detection failed</div>
            <div className="text-[11px] text-red-500/90 break-words">
              {state.message}
            </div>
          </div>
        )}

        {state.kind === 'done' && (
          <div className="space-y-1.5">
            <div className="text-[11px] text-muted-foreground">
              {state.sourceApp}
              {state.providerHost ? ` · via ${state.providerHost}` : ''}
              {' · '}
              {formatAgo(state.at)}
            </div>
            {state.participants.length === 0 ? (
              <div className="text-muted-foreground">No participants identified.</div>
            ) : (
              <ul className="space-y-0.5">
                {state.participants.map((name) => {
                  const isSpeaking =
                    state.currentSpeaker &&
                    name.toLowerCase() === state.currentSpeaker.toLowerCase();
                  return (
                    <li
                      key={name}
                      className={`flex items-center gap-1.5 ${
                        isSpeaking ? 'text-blue-600 font-medium' : 'text-gray-700'
                      }`}
                    >
                      <span
                        className={`inline-block w-1.5 h-1.5 rounded-full ${
                          isSpeaking ? 'bg-blue-500 animate-pulse' : 'bg-gray-300'
                        }`}
                      />
                      <span className="truncate">{name}</span>
                      {isSpeaking && (
                        <span className="text-[10px] text-blue-500">speaking</span>
                      )}
                    </li>
                  );
                })}
              </ul>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

function formatAgo(ts: number): string {
  const sec = Math.floor((Date.now() - ts) / 1000);
  if (sec < 5) return 'just now';
  if (sec < 60) return `${sec}s ago`;
  const min = Math.floor(sec / 60);
  return `${min}m ago`;
}
