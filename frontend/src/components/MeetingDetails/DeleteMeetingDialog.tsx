'use client';

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useRouter } from 'next/navigation';
import { toast } from 'sonner';
import { Trash2, Loader2 } from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';

interface DeleteMeetingDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  meetingId: string;
  meetingTitle: string;
  folderPath?: string | null;
}

/**
 * Destructive confirmation for permanently deleting a meeting. Two checkboxes:
 *   1. Also delete audio files from disk (default ON — previous behavior
 *      left orphaned recordings).
 *   2. "I understand this can't be undone" (default OFF — the red Delete
 *      button is disabled until this checks).
 */
export function DeleteMeetingDialog({
  open,
  onOpenChange,
  meetingId,
  meetingTitle,
  folderPath,
}: DeleteMeetingDialogProps) {
  const router = useRouter();
  const { refetchMeetings } = useSidebar();
  const [deleteAudio, setDeleteAudio] = useState(true);
  const [confirmed, setConfirmed] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);

  // Reset state every time the dialog opens.
  useEffect(() => {
    if (open) {
      setDeleteAudio(true);
      setConfirmed(false);
      setIsDeleting(false);
    }
  }, [open]);

  const handleDelete = async () => {
    if (!confirmed || isDeleting) return;
    setIsDeleting(true);
    try {
      await invoke('api_delete_meeting_with_options', {
        meetingId,
        deleteAudioFiles: deleteAudio,
      });
      toast.success('Meeting deleted', {
        description: deleteAudio
          ? 'Meeting and audio files removed.'
          : 'Meeting removed (audio files kept on disk).',
      });
      await refetchMeetings();
      onOpenChange(false);
      router.push('/');
    } catch (err) {
      console.error('Failed to delete meeting:', err);
      toast.error('Failed to delete meeting', {
        description: err instanceof Error ? err.message : String(err),
      });
      setIsDeleting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Delete meeting?</DialogTitle>
          <DialogDescription className="pt-2">
            This will permanently delete{' '}
            <span className="font-medium text-gray-900">
              &ldquo;{meetingTitle || 'Untitled meeting'}&rdquo;
            </span>{' '}
            and its transcripts. This action cannot be undone.
          </DialogDescription>
        </DialogHeader>

        <div className="mt-2 space-y-3">
          <label className="flex items-start gap-2 text-sm text-gray-700">
            <input
              type="checkbox"
              checked={deleteAudio}
              onChange={(e) => setDeleteAudio(e.target.checked)}
              className="mt-0.5 h-4 w-4 rounded border-gray-300"
              disabled={isDeleting}
            />
            <span>
              <span className="font-medium text-gray-900">
                Also delete audio files from disk
              </span>
              {folderPath && (
                <span className="mt-0.5 block break-all font-mono text-[11px] text-gray-500">
                  {folderPath}
                </span>
              )}
            </span>
          </label>

          <label className="flex items-start gap-2 text-sm text-gray-700">
            <input
              type="checkbox"
              checked={confirmed}
              onChange={(e) => setConfirmed(e.target.checked)}
              className="mt-0.5 h-4 w-4 rounded border-gray-300"
              disabled={isDeleting}
            />
            <span>I understand this can&apos;t be undone</span>
          </label>
        </div>

        <DialogFooter className="mt-4 gap-2">
          <Button
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={isDeleting}
          >
            Cancel
          </Button>
          <Button
            variant="destructive"
            onClick={handleDelete}
            disabled={!confirmed || isDeleting}
            className="bg-red-600 hover:bg-red-700"
          >
            {isDeleting ? (
              <>
                <Loader2 className="mr-1.5 h-4 w-4 animate-spin" />
                Deleting…
              </>
            ) : (
              <>
                <Trash2 className="mr-1.5 h-4 w-4" />
                Delete meeting
              </>
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
