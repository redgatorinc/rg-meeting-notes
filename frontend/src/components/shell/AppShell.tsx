'use client';

import { NavRail } from './NavRail';
import { MeetingListPane } from './MeetingListPane';

interface AppShellProps {
  children: React.ReactNode;
}

/**
 * Three-column application shell replacing the old Sidebar + MainContent pair.
 *
 *   NavRail  (60px, icons-only)
 *   MeetingListPane (280px, always mounted)
 *   <children> (flex-1, renders the routed page)
 *
 * All three columns keep their own scroll. The middle column is
 * independent of the route, so meeting rows persist across navigations.
 */
export function AppShell({ children }: AppShellProps) {
  return (
    <div className="flex h-screen w-screen overflow-hidden bg-white">
      <NavRail />
      <MeetingListPane />
      <main className="flex-1 overflow-y-auto">{children}</main>
    </div>
  );
}
