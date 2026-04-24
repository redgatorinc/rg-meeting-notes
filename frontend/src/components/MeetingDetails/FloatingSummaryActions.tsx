'use client';

import { Button } from '@/components/ui/button';
import { Copy, Save, Loader2 } from 'lucide-react';

interface FloatingSummaryActionsProps {
  isSaving: boolean;
  isDirty: boolean;
  hasSummary: boolean;
  onSave: () => Promise<void>;
  onCopy: () => Promise<void>;
}

/**
 * Absolute-positioned Save + Copy icon buttons pinned to the top-right of
 * the summary editor canvas. Replaces the flat `SummaryUpdaterButtonGroup`
 * that used to sit inline next to the generation controls.
 */
export function FloatingSummaryActions({
  isSaving,
  isDirty,
  hasSummary,
  onSave,
  onCopy,
}: FloatingSummaryActionsProps) {
  return (
    <div className="absolute right-6 top-2 z-10 flex items-center gap-1 rounded-md border border-gray-200 bg-white/95 p-1 shadow-sm backdrop-blur">
      <Button
        variant="ghost"
        size="icon"
        onClick={() => void onSave()}
        disabled={isSaving || !isDirty}
        title={isDirty ? 'Save changes' : 'No unsaved changes'}
        className={`h-7 w-7 ${isDirty ? 'text-green-600 hover:text-green-700' : 'text-gray-400'}`}
      >
        {isSaving ? <Loader2 className="h-4 w-4 animate-spin" /> : <Save className="h-4 w-4" />}
      </Button>
      <Button
        variant="ghost"
        size="icon"
        onClick={() => void onCopy()}
        disabled={!hasSummary}
        title={hasSummary ? 'Copy summary' : 'No summary to copy'}
        className="h-7 w-7 text-gray-600 hover:text-gray-900"
      >
        <Copy className="h-4 w-4" />
      </Button>
    </div>
  );
}
