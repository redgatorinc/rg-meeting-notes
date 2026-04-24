'use client';

import { useDiarizationStatus } from '@/hooks/useDiarizationStatus';
import { Loader2, CheckCircle2, AlertCircle } from 'lucide-react';

interface DiarizationStatusChipProps {
  meetingId: string;
}

/**
 * Compact status pill rendered in the meeting header while / after
 * diarization runs for a specific meeting. Silent (renders null) in the
 * `idle` state so the header chrome stays clean for meetings that haven't
 * been diarized.
 */
export function DiarizationStatusChip({ meetingId }: DiarizationStatusChipProps) {
  const status = useDiarizationStatus(meetingId);

  if (status.state === 'idle') return null;

  if (status.state === 'downloading') {
    const pct = Math.round((status.progress ?? 0) * 100);
    return (
      <Chip className="bg-blue-50 text-blue-700">
        <Loader2 className="h-3 w-3 animate-spin" />
        Downloading pack… {pct}%
      </Chip>
    );
  }

  if (status.state === 'running') {
    const pct = Math.round((status.progress ?? 0) * 100);
    return (
      <Chip className="bg-blue-50 text-blue-700">
        <Loader2 className="h-3 w-3 animate-spin" />
        Diarizing… {pct}%
      </Chip>
    );
  }

  if (status.state === 'done') {
    return (
      <Chip className="bg-green-50 text-green-700">
        <CheckCircle2 className="h-3 w-3" />
        {status.speaker_count} speaker{status.speaker_count === 1 ? '' : 's'}
      </Chip>
    );
  }

  if (status.state === 'error') {
    return (
      <Chip className="bg-red-50 text-red-700" title={status.message}>
        <AlertCircle className="h-3 w-3" />
        Diarization failed
      </Chip>
    );
  }

  // Exhaustive switch above — state union covers every case. This
  // fallthrough only runs if a new variant is added without updating the
  // chip.
  return null;
}

function Chip({
  className = '',
  children,
  title,
}: {
  className?: string;
  children: React.ReactNode;
  title?: string;
}) {
  return (
    <span
      title={title}
      className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-medium ${className}`}
    >
      {children}
    </span>
  );
}
