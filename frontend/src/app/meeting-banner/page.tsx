'use client';

import { Suspense } from 'react';
import { useSearchParams } from 'next/navigation';
import { invoke } from '@tauri-apps/api/core';
import { Mic, X } from 'lucide-react';

function BannerContent() {
  const searchParams = useSearchParams();
  const appName = searchParams.get('app') || 'Meeting';

  const handleStart = async () => {
    try {
      await invoke('accept_meeting_banner');
    } catch (e) {
      console.error('Failed to accept banner:', e);
    }
  };

  const handleDismiss = async () => {
    try {
      await invoke('dismiss_meeting_banner');
    } catch (e) {
      console.error('Failed to dismiss banner:', e);
    }
  };

  return (
    <div
      className="w-full h-full flex items-center justify-center select-none"
      style={{ background: 'transparent' }}
      data-tauri-drag-region
    >
      <div
        className="flex items-center gap-3 bg-white text-slate-900 rounded-full pl-4 pr-2 py-2 border border-slate-200 shadow-lg"
        style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
      >
        {/* App icon */}
        <div className="flex-shrink-0 w-9 h-9 rounded-full bg-slate-100 flex items-center justify-center">
          <img
            src="/redgator-icon.png"
            alt="Redgator"
            className="w-6 h-6 rounded-sm"
            draggable={false}
          />
        </div>

        {/* Text */}
        <div className="flex flex-col leading-tight mr-1">
          <span className="text-[13px] font-semibold whitespace-nowrap text-slate-900">
            Start AI Notes
          </span>
          <span className="text-[11px] text-slate-500 whitespace-nowrap">
            {appName} meeting detected
          </span>
        </div>

        {/* Start button */}
        <button
          onClick={handleStart}
          className="flex items-center gap-1.5 bg-blue-500 hover:bg-blue-600 active:bg-blue-700 text-white text-[13px] font-medium rounded-full px-4 py-1.5 transition-colors whitespace-nowrap cursor-pointer"
          style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
        >
          <Mic className="w-3.5 h-3.5" />
          Start transcribing
        </button>

        {/* Dismiss */}
        <button
          onClick={handleDismiss}
          className="flex-shrink-0 p-1.5 rounded-full hover:bg-slate-100 transition-colors cursor-pointer"
          style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
        >
          <X className="w-4 h-4 text-slate-500" />
        </button>
      </div>
    </div>
  );
}

export default function MeetingBannerPage() {
  return (
    <Suspense>
      <BannerContent />
    </Suspense>
  );
}
