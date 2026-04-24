"use client";

import { Button } from '@/components/ui/button';
import { Copy } from 'lucide-react';
import Analytics from '@/lib/analytics';


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
 * Slim transcript-panel action row. Now only carries "Copy transcript" —
 * the recording-folder and retranscribe actions moved into the new
 * `MeetingHeader`. Kept as a tiny component so the TranscriptPanel's top
 * bar still has a defined affordance.
 */
export function TranscriptButtonGroup({
  transcriptCount,
  onCopyTranscript,
}: TranscriptButtonGroupProps) {
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
    </div>
  );
}
