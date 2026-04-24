'use client';

import { useRouter, usePathname } from 'next/navigation';
import { Home, Mic, Settings as SettingsIcon, Upload } from 'lucide-react';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import { useRecordingState } from '@/contexts/RecordingStateContext';
import { useConfig } from '@/contexts/ConfigContext';
import { useImportDialog } from '@/contexts/ImportDialogContext';
import Image from 'next/image';

/**
 * Thin icon-only left rail. Home / Start recording / Import / Settings.
 * Replaces the flat meetings-list nav — the list itself lives in
 * `MeetingListPane` to the right.
 */
export function NavRail() {
  const router = useRouter();
  const pathname = usePathname();
  const { handleRecordingToggle } = useSidebar();
  const { isRecording } = useRecordingState();
  const { betaFeatures } = useConfig();
  const { openImportDialog } = useImportDialog();

  const go = (path: string) => router.push(path);

  return (
    <nav className="flex h-full w-[60px] flex-col items-center border-r border-gray-200 bg-white py-3">
      {/* Brand */}
      <button
        onClick={() => go('/')}
        className="mb-4 flex h-9 w-9 items-center justify-center rounded-md hover:bg-gray-100"
        title="Home"
      >
        <Image src="/redgator-icon.png" alt="Redgator" width={28} height={28} />
      </button>

      <div className="flex flex-1 flex-col gap-1">
        <NavIcon
          icon={<Home className="h-5 w-5" />}
          label="Home"
          active={pathname === '/'}
          onClick={() => go('/')}
        />
        <NavIcon
          icon={
            <Mic className={`h-5 w-5 ${isRecording ? 'text-red-500 animate-pulse' : ''}`} />
          }
          label={isRecording ? 'Recording…' : 'Start recording'}
          onClick={handleRecordingToggle}
        />
        {betaFeatures?.importAndRetranscribe && (
          <NavIcon
            icon={<Upload className="h-5 w-5" />}
            label="Import audio"
            onClick={() => openImportDialog(null)}
          />
        )}
      </div>

      <NavIcon
        icon={<SettingsIcon className="h-5 w-5" />}
        label="Settings"
        active={pathname === '/settings'}
        onClick={() => go('/settings')}
      />
    </nav>
  );
}

function NavIcon({
  icon,
  label,
  active = false,
  onClick,
}: {
  icon: React.ReactNode;
  label: string;
  active?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      title={label}
      className={`flex h-9 w-9 items-center justify-center rounded-md transition-colors ${
        active
          ? 'bg-blue-50 text-blue-600'
          : 'text-gray-600 hover:bg-gray-100 hover:text-gray-900'
      }`}
    >
      {icon}
    </button>
  );
}
