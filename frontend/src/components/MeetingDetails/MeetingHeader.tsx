'use client';

import { useState, useEffect, useRef } from 'react';
import { FolderOpen, RefreshCw, MoreHorizontal, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { ButtonGroup } from '@/components/ui/button-group';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import Analytics from '@/lib/analytics';
import { useConfig } from '@/contexts/ConfigContext';
import { RetranscribeDialog } from './RetranscribeDialog';
import { DeleteMeetingDialog } from './DeleteMeetingDialog';
import { formatBytes, formatDate, formatDuration } from '@/lib/formatting';
import type { MeetingListItem } from '@/types';

interface MeetingHeaderProps {
  meetingId: string;
  folderPath?: string | null;
  title: string;
  createdAt?: string;
  onTitleChange: (title: string) => void;
  onOpenFolder: () => void;
  onRefetchTranscripts?: () => Promise<void>;
}

// Keep in sync with DB-side; mic=cluster 0, system=cluster 1 default palette
// (Same golden-angle hue used in VirtualizedTranscriptView for consistency.)
function colorForCluster(idx: number): string {
  const hue = (idx * 137) % 360;
  return `hsl(${hue}, 70%, 42%)`;
}

/**
 * Inline meeting header — compact panel above the transcript/summary split.
 * Shows title (editable), date/duration/speakers/size meta, speaker chips,
 * and the action bar (open folder / retranscribe / delete).
 */
export function MeetingHeader({
  meetingId,
  folderPath,
  title,
  createdAt,
  onTitleChange,
  onOpenFolder,
  onRefetchTranscripts,
}: MeetingHeaderProps) {
  const { betaFeatures } = useConfig();
  const [showRetranscribe, setShowRetranscribe] = useState(false);
  const [showDelete, setShowDelete] = useState(false);

  // Enriched metadata fetched separately — cheap, single round-trip to the
  // list endpoint that already carries duration / speakers / size.
  const [meta, setMeta] = useState<MeetingListItem | null>(null);
  useEffect(() => {
    let disposed = false;
    const load = async () => {
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        const rows = await invoke<MeetingListItem[]>('api_get_meetings');
        if (disposed) return;
        setMeta(rows.find((r) => r.id === meetingId) ?? null);
      } catch {
        if (!disposed) setMeta(null);
      }
    };
    void load();
  }, [meetingId, title]);

  // Speakers panel for this meeting — separate round-trip (small query).
  const [speakers, setSpeakers] = useState<Array<{
    id: string;
    display_name: string | null;
    cluster_idx: number;
    total_speaking_ms: number;
  }>>([]);
  useEffect(() => {
    let disposed = false;
    const load = async () => {
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        const rows = await invoke<typeof speakers>('speakers_list', {
          meetingId,
        }).catch(() => [] as typeof speakers);
        if (!disposed) setSpeakers(rows ?? []);
      } catch {
        if (!disposed) setSpeakers([]);
      }
    };
    void load();
  }, [meetingId]);

  const totalSpeakingMs = speakers.reduce((sum, s) => sum + (s.total_speaking_ms || 0), 0);

  return (
    <div className="border-b border-gray-200 bg-white px-6 py-4">
      <div className="flex items-start justify-between gap-4">
        {/* Title + meta */}
        <div className="min-w-0 flex-1">
          <TitleField title={title} onChange={onTitleChange} />
          <MetaRow
            createdAt={createdAt}
            durationMs={meta?.duration_ms ?? 0}
            speakerCount={meta?.speaker_count ?? speakers.length}
            fileSizeBytes={meta?.file_size_bytes ?? 0}
          />
        </div>

        {/* Actions */}
        <div className="flex flex-shrink-0 items-center gap-1">
          <ButtonGroup>
            <Button
              size="sm"
              variant="outline"
              onClick={() => {
                Analytics.trackButtonClick('open_recording_folder', 'meeting_header');
                onOpenFolder();
              }}
              title="Open recordings folder"
            >
              <FolderOpen className="h-4 w-4" />
            </Button>
            {betaFeatures?.importAndRetranscribe && folderPath && (
              <Button
                size="sm"
                variant="outline"
                onClick={() => {
                  Analytics.trackButtonClick('retranscribe', 'meeting_header');
                  setShowRetranscribe(true);
                }}
                title="Retranscribe this recording"
                className="gap-1.5"
              >
                <RefreshCw className="h-4 w-4" />
                <span className="hidden lg:inline">Retranscribe</span>
              </Button>
            )}
          </ButtonGroup>

          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button size="sm" variant="ghost" title="More">
                <MoreHorizontal className="h-4 w-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem
                onClick={() => setShowDelete(true)}
                className="text-red-600 focus:bg-red-50 focus:text-red-700"
              >
                <Trash2 className="mr-2 h-4 w-4" />
                Delete meeting
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>

      {/* Speaker chips */}
      {speakers.length > 0 && totalSpeakingMs > 0 && (
        <div className="mt-3 flex flex-wrap gap-2">
          {speakers
            .slice()
            .sort((a, b) => b.total_speaking_ms - a.total_speaking_ms)
            .map((s) => {
              const pct = totalSpeakingMs > 0
                ? Math.round((s.total_speaking_ms / totalSpeakingMs) * 100)
                : 0;
              const label = (s.display_name?.trim() || `Speaker ${s.cluster_idx + 1}`);
              return (
                <span
                  key={s.id}
                  className="inline-flex items-center gap-1.5 rounded-full border border-gray-200 bg-gray-50 px-2.5 py-0.5 text-xs"
                >
                  <span
                    className="inline-block h-2 w-2 rounded-full"
                    style={{ backgroundColor: colorForCluster(s.cluster_idx) }}
                  />
                  <span className="font-medium text-gray-900">{label}</span>
                  <span className="text-gray-500">{pct}%</span>
                </span>
              );
            })}
        </div>
      )}

      {/* Dialogs */}
      {betaFeatures?.importAndRetranscribe && folderPath && (
        <RetranscribeDialog
          open={showRetranscribe}
          onOpenChange={setShowRetranscribe}
          meetingId={meetingId}
          meetingFolderPath={folderPath}
          onComplete={onRefetchTranscripts}
        />
      )}
      <DeleteMeetingDialog
        open={showDelete}
        onOpenChange={setShowDelete}
        meetingId={meetingId}
        meetingTitle={title}
        folderPath={folderPath ?? null}
      />
    </div>
  );
}

function TitleField({
  title,
  onChange,
}: {
  title: string;
  onChange: (title: string) => void;
}) {
  const [draft, setDraft] = useState(title);
  const [editing, setEditing] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!editing) setDraft(title);
  }, [title, editing]);

  const commit = () => {
    const trimmed = draft.trim();
    if (trimmed && trimmed !== title) onChange(trimmed);
    setEditing(false);
  };

  if (editing) {
    return (
      <input
        ref={inputRef}
        autoFocus
        type="text"
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === 'Enter') e.currentTarget.blur();
          if (e.key === 'Escape') {
            setDraft(title);
            setEditing(false);
          }
        }}
        className="w-full border-b border-blue-400 bg-transparent text-xl font-semibold text-gray-900 outline-none"
      />
    );
  }

  return (
    <button
      type="button"
      onClick={() => setEditing(true)}
      className="group block w-full truncate text-left text-xl font-semibold text-gray-900 hover:text-blue-700"
      title="Click to rename"
    >
      {title || 'Untitled meeting'}
    </button>
  );
}

function MetaRow({
  createdAt,
  durationMs,
  speakerCount,
  fileSizeBytes,
}: {
  createdAt?: string;
  durationMs: number;
  speakerCount: number;
  fileSizeBytes: number;
}) {
  const parts: string[] = [];
  if (createdAt) parts.push(formatDate(createdAt));
  if (durationMs > 0) parts.push(formatDuration(durationMs));
  if (speakerCount > 0) parts.push(`${speakerCount} speaker${speakerCount === 1 ? '' : 's'}`);
  if (fileSizeBytes > 0) parts.push(formatBytes(fileSizeBytes));
  if (parts.length === 0) return null;
  return (
    <div className="mt-1 text-xs text-gray-500">{parts.join(' · ')}</div>
  );
}
