"use client";

import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import { Button } from '@/components/ui/button';
import { Copy, Users, Loader2 } from 'lucide-react';
import Analytics from '@/lib/analytics';
import { useDiarizationStatus } from '@/hooks/useDiarizationStatus';


interface TranscriptButtonGroupProps {
  transcriptCount: number;
  onCopyTranscript: () => void;
  /** Deprecated: folder open + retranscribe moved to MeetingHeader. Kept
   *  in the prop surface so the existing PageContent prop wiring compiles
   *  without churn — they are unused here. */
  onOpenMeetingFolder?: () => Promise<void>;
  meetingId?: string;
  meetingFolderPath?: string | null;
  onRefetchTranscripts?: () => Promise<void>;
}


/**
 * Slim transcript-panel action row. Carries Copy + Diarize. Retranscribe
 * + Open-folder + Delete live in `MeetingHeader`.
 */
export function TranscriptButtonGroup({
  transcriptCount,
  onCopyTranscript,
  meetingId,
}: TranscriptButtonGroupProps) {
  const [triggering, setTriggering] = useState(false);
  const status = useDiarizationStatus(meetingId ?? null);
  const running =
    status.state === 'running' ||
    status.state === 'downloading' ||
    triggering;

  const handleDiarize = async () => {
    if (!meetingId || running) return;
    setTriggering(true);
    try {
      const { Store } = await import('@tauri-apps/plugin-store');
      const store = await Store.load('preferences.json');
      const pack = (await store.get<string>('diarization.model_pack')) ?? 'default';
      Analytics.trackButtonClick('diarize_from_transcript', 'meeting_details');
      await invoke('diarization_start', { meetingId, pack });
    } catch (err) {
      console.error('diarization_start failed:', err);
      toast.error('Diarize failed', {
        description: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setTriggering(false);
    }
  };

  return (
    <div className="flex items-center justify-end w-full gap-2">
      <Button
        variant="outline"
        size="sm"
        onClick={() => {
          Analytics.trackButtonClick('copy_transcript', 'meeting_details');
          onCopyTranscript();
        }}
        disabled={transcriptCount === 0}
        title={transcriptCount === 0 ? 'No transcript available' : 'Copy Transcript'}
      >
        <Copy className="h-4 w-4" />
        <span className="ml-1.5 hidden lg:inline">Copy</span>
      </Button>

      {meetingId && (
        <Button
          variant="outline"
          size="sm"
          onClick={() => void handleDiarize()}
          disabled={transcriptCount === 0 || running}
          title={
            transcriptCount === 0
              ? 'No transcript to diarize'
              : running
                ? 'Diarization in progress'
                : 'Identify speakers'
          }
        >
          {running ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Users className="h-4 w-4" />
          )}
          <span className="ml-1.5 hidden lg:inline">
            {running ? 'Diarizing…' : 'Diarize'}
          </span>
        </Button>
      )}
    </div>
  );
}
