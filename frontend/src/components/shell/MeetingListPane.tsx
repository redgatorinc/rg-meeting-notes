'use client';

import { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Search, Mic, Upload } from 'lucide-react';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import { useConfig } from '@/contexts/ConfigContext';
import { useImportDialog } from '@/contexts/ImportDialogContext';
import { MeetingListItem } from './MeetingListItem';
import type { MeetingListItem as MeetingRow } from '@/types';

/**
 * Always-mounted middle column that lists every meeting.
 * Takes over the role the old monolithic Sidebar played for meeting
 * discovery. Fetches the enriched shape from `api_get_meetings`
 * (duration_ms, speaker_count, file_size_bytes) and renders via
 * `MeetingListItem`.
 */
export function MeetingListPane() {
  const { handleRecordingToggle, refetchMeetings, meetings } = useSidebar();
  const { betaFeatures } = useConfig();
  const { openImportDialog } = useImportDialog();
  const [rich, setRich] = useState<MeetingRow[]>([]);
  const [query, setQuery] = useState('');

  // Fetch the enriched rows directly; `meetings` on SidebarProvider still
  // carries the minimal {id,title} shape for legacy consumers. We keep the
  // two in sync on refetch.
  useEffect(() => {
    let disposed = false;
    const load = async () => {
      try {
        const rows = await invoke<MeetingRow[]>('api_get_meetings');
        if (!disposed) setRich(rows ?? []);
      } catch {
        if (!disposed) setRich([]);
      }
    };
    void load();
    // Re-fetch whenever the legacy count changes (a new meeting was saved,
    // or a delete happened).
  }, [meetings.length]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return rich;
    return rich.filter((m) => m.title.toLowerCase().includes(q));
  }, [rich, query]);

  const empty = filtered.length === 0;
  const isEmptyBecauseNoMeetings = rich.length === 0;

  return (
    <aside className="flex h-full w-[280px] flex-col border-r border-gray-200 bg-white">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-gray-200 px-3 py-3">
        <h2 className="text-sm font-semibold text-gray-900">Meetings</h2>
        <span className="text-[11px] text-gray-500">{rich.length}</span>
      </div>

      {/* Search */}
      <div className="border-b border-gray-200 px-3 py-2">
        <div className="relative">
          <Search className="pointer-events-none absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-gray-400" />
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search meetings"
            className="w-full rounded-md border border-gray-200 bg-gray-50 py-1.5 pl-7 pr-2 text-xs focus:border-blue-500 focus:bg-white focus:outline-none focus:ring-1 focus:ring-blue-500"
          />
        </div>
      </div>

      {/* List */}
      <div className="flex-1 overflow-y-auto">
        {empty ? (
          <EmptyState
            showActions={isEmptyBecauseNoMeetings}
            onStartRecording={handleRecordingToggle}
            onImport={betaFeatures?.importAndRetranscribe ? () => openImportDialog(null) : undefined}
            query={query}
          />
        ) : (
          filtered.map((meeting) => <MeetingListItem key={meeting.id} meeting={meeting} />)
        )}
      </div>

      {/* Footer refresh hint */}
      <button
        type="button"
        onClick={() => void refetchMeetings()}
        className="border-t border-gray-200 px-3 py-2 text-left text-[11px] text-gray-500 hover:bg-gray-50 hover:text-gray-700"
      >
        Refresh list
      </button>
    </aside>
  );
}

function EmptyState({
  showActions,
  onStartRecording,
  onImport,
  query,
}: {
  showActions: boolean;
  onStartRecording: () => void;
  onImport?: () => void;
  query: string;
}) {
  if (!showActions) {
    return (
      <div className="px-3 py-6 text-center text-xs text-gray-500">
        No meetings match &ldquo;{query}&rdquo;.
      </div>
    );
  }

  return (
    <div className="px-3 py-6 text-center">
      <div className="mb-1 text-sm font-semibold text-gray-900">No meetings yet</div>
      <p className="mb-3 text-xs text-gray-500">
        Record your first meeting or import an audio file to get started.
      </p>
      <div className="flex flex-col gap-2">
        <button
          onClick={onStartRecording}
          className="inline-flex items-center justify-center gap-1.5 rounded-md bg-red-500 px-3 py-1.5 text-xs font-medium text-white hover:bg-red-600"
        >
          <Mic className="h-3.5 w-3.5" />
          Start recording
        </button>
        {onImport && (
          <button
            onClick={onImport}
            className="inline-flex items-center justify-center gap-1.5 rounded-md border border-gray-200 bg-white px-3 py-1.5 text-xs font-medium text-gray-700 hover:bg-gray-50"
          >
            <Upload className="h-3.5 w-3.5" />
            Import audio
          </button>
        )}
      </div>
    </div>
  );
}
