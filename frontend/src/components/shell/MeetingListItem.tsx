'use client';

import { useRouter, useSearchParams } from 'next/navigation';
import { Users, Clock, HardDrive } from 'lucide-react';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import { formatDate, formatDuration, formatBytes } from '@/lib/formatting';
import type { MeetingListItem as MeetingRow } from '@/types';

interface MeetingListItemProps {
  meeting: MeetingRow;
}

export function MeetingListItem({ meeting }: MeetingListItemProps) {
  const router = useRouter();
  const searchParams = useSearchParams();
  const { setCurrentMeeting } = useSidebar();

  const isActive = searchParams.get('id') === meeting.id;

  const handleClick = () => {
    setCurrentMeeting({ id: meeting.id, title: meeting.title });
    router.push(`/meeting-details?id=${meeting.id}`);
  };

  const duration = meeting.duration_ms ?? 0;
  const speakers = meeting.speaker_count ?? 0;
  const size = meeting.file_size_bytes ?? 0;

  return (
    <button
      onClick={handleClick}
      className={`w-full cursor-pointer border-b border-gray-100 px-3 py-2.5 text-left transition-colors ${
        isActive ? 'bg-blue-50' : 'hover:bg-gray-50'
      }`}
    >
      <div className="truncate text-sm font-semibold text-gray-900" title={meeting.title}>
        {meeting.title || 'Untitled meeting'}
      </div>
      {meeting.created_at && (
        <div className="mt-0.5 text-[11px] text-gray-500">{formatDate(meeting.created_at)}</div>
      )}
      <div className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-[11px] text-gray-500">
        {duration > 0 && (
          <span className="inline-flex items-center gap-1">
            <Clock className="h-3 w-3" />
            {formatDuration(duration)}
          </span>
        )}
        {speakers > 0 && (
          <span className="inline-flex items-center gap-1">
            <Users className="h-3 w-3" />
            {speakers}
          </span>
        )}
        {size > 0 && (
          <span className="inline-flex items-center gap-1">
            <HardDrive className="h-3 w-3" />
            {formatBytes(size)}
          </span>
        )}
      </div>
    </button>
  );
}
