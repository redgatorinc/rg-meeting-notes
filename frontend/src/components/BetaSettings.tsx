"use client"

import { Switch } from "./ui/switch"
import { FlaskConical, AlertCircle } from "lucide-react"
import { useConfig } from "@/contexts/ConfigContext"
import {
  BetaFeatureKey,
  BETA_FEATURE_NAMES,
  BETA_FEATURE_DESCRIPTIONS
} from "@/types/betaFeatures"
import {
  SettingsNotice,
  SettingsSubsectionTitle,
  SettingsTogglePanel,
} from "@/components/settings/SettingsPanel"

export function BetaSettings() {
  const { betaFeatures, toggleBetaFeature } = useConfig();

  // Define feature order for display (allows custom ordering)
  const featureOrder: BetaFeatureKey[] = ['importAndRetranscribe'];

  return (
    <div className="space-y-6">
      {/* Yellow Warning Banner */}
      <SettingsNotice tone="warning" className="flex items-start gap-3">
        <AlertCircle className="h-5 w-5 text-yellow-600 flex-shrink-0 mt-0.5" />
        <div className="text-sm text-yellow-800">
          <SettingsSubsectionTitle className="text-yellow-900">Beta Features</SettingsSubsectionTitle>
          <p className="mt-1">
            These features are still being tested. You may encounter issues, and we appreciate your feedback.
          </p>
        </div>
      </SettingsNotice>

      {/* Dynamic Feature Toggles - Automatically renders all features */}
      {featureOrder.map((featureKey) => (
        <SettingsTogglePanel
          key={featureKey}
          title={
            <span className="flex items-center gap-2">
              <FlaskConical className="h-5 w-5 text-gray-600" />
              <span>{BETA_FEATURE_NAMES[featureKey]}</span>
              <span className="rounded-full bg-yellow-100 px-2 py-0.5 text-xs font-medium text-yellow-800">
                BETA
              </span>
            </span>
          }
          description={BETA_FEATURE_DESCRIPTIONS[featureKey]}
          control={
              <Switch
                checked={betaFeatures[featureKey]}
                onCheckedChange={(checked) => toggleBetaFeature(featureKey, checked)}
              />
          }
        />
      ))}

      {/* Info Box */}
      <SettingsNotice tone="info">
        <p className="text-sm text-blue-800">
          <strong>Note:</strong> When disabled, beta features will be hidden. Your existing meetings remain unaffected.
        </p>
      </SettingsNotice>
    </div>
  );
}
