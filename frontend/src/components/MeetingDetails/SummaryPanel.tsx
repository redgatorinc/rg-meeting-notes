"use client";

import { Summary, SummaryResponse, Transcript } from '@/types';
import { BlockNoteSummaryView, BlockNoteSummaryViewRef } from '@/components/AISummary/BlockNoteSummaryView';
import { EmptyStateSummary } from '@/components/EmptyStateSummary';
import { ModelConfig } from '@/components/ModelSettingsModal';
import { SummaryGenerationCard } from './SummaryGenerationCard';
import { FloatingSummaryActions } from './FloatingSummaryActions';
import Analytics from '@/lib/analytics';
import { RefObject } from 'react';

interface SummaryPanelProps {
  meeting: {
    id: string;
    title: string;
    created_at: string;
  };
  meetingTitle: string;
  onTitleChange: (title: string) => void;
  isEditingTitle: boolean;
  onStartEditTitle: () => void;
  onFinishEditTitle: () => void;
  isTitleDirty: boolean;
  summaryRef: RefObject<BlockNoteSummaryViewRef>;
  isSaving: boolean;
  onSaveAll: () => Promise<void>;
  onCopySummary: () => Promise<void>;
  onOpenFolder: () => Promise<void>;
  aiSummary: Summary | null;
  summaryStatus: 'idle' | 'processing' | 'summarizing' | 'regenerating' | 'completed' | 'error';
  transcripts: Transcript[];
  modelConfig: ModelConfig;
  setModelConfig: (config: ModelConfig | ((prev: ModelConfig) => ModelConfig)) => void;
  onSaveModelConfig: (config?: ModelConfig) => Promise<void>;
  onGenerateSummary: (customPrompt: string) => Promise<void>;
  onStopGeneration: () => void;
  customPrompt: string;
  summaryResponse: SummaryResponse | null;
  onSaveSummary: (summary: Summary | { markdown?: string; summary_json?: any[] }) => Promise<void>;
  onSummaryChange: (summary: Summary) => void;
  onDirtyChange: (isDirty: boolean) => void;
  summaryError: string | null;
  onRegenerateSummary: () => Promise<void>;
  getSummaryStatusMessage: (status: 'idle' | 'processing' | 'summarizing' | 'regenerating' | 'completed' | 'error') => string;
  availableTemplates: Array<{ id: string, name: string, description: string, is_custom: boolean }>;
  selectedTemplate: string;
  onTemplateSelect: (templateId: string, templateName: string) => void;
  onFetchTemplateDetails?: (templateId: string) => Promise<any>;
  onSaveTemplate?: (templateId: string, data: any) => Promise<void>;
  onDeleteTemplate?: (templateId: string) => Promise<void>;
  isModelConfigLoading?: boolean;
  onOpenModelSettings?: (openFn: () => void) => void;
}

export function SummaryPanel({
  meeting,
  meetingTitle,
  isTitleDirty,
  summaryRef,
  isSaving,
  onSaveAll,
  onCopySummary,
  aiSummary,
  summaryStatus,
  transcripts,
  modelConfig,
  setModelConfig,
  onSaveModelConfig,
  onGenerateSummary,
  onStopGeneration,
  customPrompt,
  onSaveSummary,
  onSummaryChange,
  onDirtyChange,
  summaryError,
  onRegenerateSummary,
  getSummaryStatusMessage,
  availableTemplates,
  selectedTemplate,
  onTemplateSelect,
  onFetchTemplateDetails,
  onSaveTemplate,
  onDeleteTemplate,
  isModelConfigLoading = false,
  onOpenModelSettings,
}: SummaryPanelProps) {
  const isSummaryLoading =
    summaryStatus === 'processing' ||
    summaryStatus === 'summarizing' ||
    summaryStatus === 'regenerating';

  const hasSummary = !!aiSummary;
  const editorDirty = isTitleDirty || (summaryRef.current?.isDirty || false);

  return (
    <div className="flex min-w-0 flex-1 flex-col overflow-hidden bg-white">
      {/* Generation controls card — always visible so the user can swap
          model / prompt and re-generate even when a summary already exists. */}
      <SummaryGenerationCard
        modelConfig={modelConfig}
        setModelConfig={setModelConfig}
        onSaveModelConfig={onSaveModelConfig}
        onGenerateSummary={onGenerateSummary}
        onStopGeneration={onStopGeneration}
        customPrompt={customPrompt}
        summaryStatus={summaryStatus}
        availableTemplates={availableTemplates}
        selectedTemplate={selectedTemplate}
        onTemplateSelect={onTemplateSelect}
        onFetchTemplateDetails={onFetchTemplateDetails}
        onSaveTemplate={onSaveTemplate}
        onDeleteTemplate={onDeleteTemplate}
        hasTranscripts={transcripts.length > 0}
        isModelConfigLoading={isModelConfigLoading}
        onOpenModelSettings={onOpenModelSettings}
      />

      {/* Content area */}
      <div className="relative flex-1 overflow-hidden">
        {isSummaryLoading ? (
          <div className="flex h-full items-center justify-center">
            <div className="text-center">
              <div className="mx-auto mb-4 h-12 w-12 animate-spin rounded-full border-b-2 border-t-2 border-blue-500" />
              <p className="text-gray-600">Generating AI Summary…</p>
            </div>
          </div>
        ) : !hasSummary ? (
          <EmptyStateSummary
            onGenerate={() => onGenerateSummary(customPrompt)}
            hasModel={modelConfig.provider !== null && modelConfig.model !== null}
            isGenerating={isSummaryLoading}
          />
        ) : (
          <>
            <FloatingSummaryActions
              isSaving={isSaving}
              isDirty={editorDirty}
              hasSummary={hasSummary}
              onSave={onSaveAll}
              onCopy={onCopySummary}
            />
            <div className="h-full overflow-y-auto">
              <div className="w-full p-6 pt-10">
                <BlockNoteSummaryView
                  ref={summaryRef}
                  summaryData={aiSummary}
                  onSave={onSaveSummary}
                  onSummaryChange={onSummaryChange}
                  onDirtyChange={onDirtyChange}
                  status={summaryStatus}
                  error={summaryError}
                  onRegenerateSummary={() => {
                    Analytics.trackButtonClick('regenerate_summary', 'meeting_details');
                    onRegenerateSummary();
                  }}
                  meeting={{
                    id: meeting.id,
                    title: meetingTitle,
                    created_at: meeting.created_at,
                  }}
                />
              </div>
              {summaryStatus !== 'idle' && (
                <div
                  className={`mx-6 mb-6 rounded-lg p-4 ${
                    summaryStatus === 'error'
                      ? 'bg-red-100 text-red-700'
                      : summaryStatus === 'completed'
                        ? 'bg-green-100 text-green-700'
                        : 'bg-blue-100 text-blue-700'
                  }`}
                >
                  <p className="text-sm font-medium">{getSummaryStatusMessage(summaryStatus)}</p>
                </div>
              )}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
