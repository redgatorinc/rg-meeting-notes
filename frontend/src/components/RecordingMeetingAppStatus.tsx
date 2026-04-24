'use client';

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Radio, X } from 'lucide-react';

interface LockedAdapter {
  app_id: string;
  app_display_name: string;
  method: string;
}

interface ActiveSession {
  meeting_id: string | null;
  locked_adapter: LockedAdapter | null;
  detected_at: string;
}

interface RecordingMeetingAppStatusProps {
  visible: boolean;
}

/**
 * Small top-left-of-transcript card showing which meeting app we
 * detected at recording start, and which detection method is locked
 * in for this session. Listens for `recording-app-detected` events
 * so it updates instantly when the session is (re)detected.
 */
export function RecordingMeetingAppStatus({ visible }: RecordingMeetingAppStatusProps) {
  const [session, setSession] = useState<ActiveSession | null>(null);
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    if (!visible) {
      setDismissed(false);
      return;
    }
    // Initial read in case the event fired before we mounted.
    void invoke<ActiveSession | null>('participant_session_info').then(setSession);
  }, [visible]);

  useEffect(() => {
    if (!visible) return;
    let cleanup: (() => void) | undefined;
    (async () => {
      cleanup = await listen<ActiveSession>('recording-app-detected', (e) => {
        setSession(e.payload);
        setDismissed(false);
      });
    })();
    return () => cleanup?.();
  }, [visible]);

  if (!visible || dismissed || !session?.locked_adapter) return null;
  const a = session.locked_adapter;
  const isAi = a.app_id === 'ai';

  return (
    <div className="fixed top-20 right-4 z-40 w-72 max-w-[90vw] rounded-lg border bg-white shadow-md text-xs">
      <div className="flex items-center justify-between border-b px-3 py-1.5">
        <div className="flex items-center gap-1.5 font-medium text-gray-800">
          <Radio className="h-3.5 w-3.5 text-red-500 animate-pulse" />
          <span>Recording session</span>
        </div>
        <button
          type="button"
          onClick={() => setDismissed(true)}
          className="text-gray-500 hover:text-gray-800"
          title="Hide"
        >
          <X className="h-3.5 w-3.5" />
        </button>
      </div>
      <div className="px-3 py-2 space-y-0.5">
        <div>
          <span className="text-muted-foreground">App: </span>
          <span className="font-medium">{a.app_display_name}</span>
        </div>
        <div>
          <span className="text-muted-foreground">Method: </span>
          <span>{isAi ? 'Vision AI (fallback)' : `${methodLabel(a.method)} (Beta)`}</span>
        </div>
        {isAi && (
          <p className="text-[11px] text-amber-600 mt-1">
            No integrated adapter matched. Participant detection will use the
            screenshot + vision path.
          </p>
        )}
      </div>
    </div>
  );
}

function methodLabel(method: string): string {
  switch (method) {
    case 'log_tail':
      return 'Log tail';
    case 'a11y':
      return 'Accessibility tree';
    case 'extension_bridge':
      return 'Browser extension';
    default:
      return method;
  }
}
