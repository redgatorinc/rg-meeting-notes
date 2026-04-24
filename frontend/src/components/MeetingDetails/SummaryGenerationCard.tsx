'use client';

import type { ModelConfig } from '@/components/ModelSettingsModal';
import {
  SettingsPanel,
  SettingsPanelHeader,
} from '@/components/settings/SettingsPanel';
import { SummaryGeneratorButtonGroup } from './SummaryGeneratorButtonGroup';

interface SummaryGenerationCardProps {
  modelConfig: ModelConfig;
  setModelConfig: (config: ModelConfig | ((prev: ModelConfig) => ModelConfig)) => void;
  onSaveModelConfig: (config?: ModelConfig) => Promise<void>;
  onGenerateSummary: (customPrompt: string) => Promise<void>;
  onStopGeneration: () => void;
  customPrompt: string;
  summaryStatus: 'idle' | 'processing' | 'summarizing' | 'regenerating' | 'completed' | 'error';
  availableTemplates: Array<{ id: string; name: string; description: string; is_custom: boolean }>;
  selectedTemplate: string;
  onTemplateSelect: (templateId: string, templateName: string) => void;
  onFetchTemplateDetails?: (templateId: string) => Promise<any>;
  onSaveTemplate?: (templateId: string, data: any) => Promise<void>;
  onDeleteTemplate?: (templateId: string) => Promise<void>;
  hasTranscripts: boolean;
  isModelConfigLoading?: boolean;
  onOpenModelSettings?: (openFn: () => void) => void;
}

/**
 * Titled card that groups the Model + Prompt template + Generate controls
 * into one visual workflow. Replaces the flat horizontal row of three
 * isolated buttons at the top of SummaryPanel.
 */
export function SummaryGenerationCard(props: SummaryGenerationCardProps) {
  return (
    <SettingsPanel className="mx-6 mb-3 mt-4 !p-4">
      <SettingsPanelHeader
        title="Summary Generation"
        description="Pick a model and a prompt template, then generate."
        className="mb-3"
      />
      <div className="flex flex-wrap items-center gap-2">
        <SummaryGeneratorButtonGroup {...props} />
      </div>
    </SettingsPanel>
  );
}
