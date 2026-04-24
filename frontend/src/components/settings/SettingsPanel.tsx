import type { ReactNode } from 'react';
import { cn } from '@/lib/utils';

interface SettingsPanelProps {
  children: ReactNode;
  className?: string;
}

interface SettingsPanelHeaderProps {
  title: ReactNode;
  description?: ReactNode;
  className?: string;
}

interface SettingsFieldProps extends SettingsPanelHeaderProps {
  children: ReactNode;
  contentClassName?: string;
}

interface SettingsTogglePanelProps {
  title: ReactNode;
  description?: ReactNode;
  control: ReactNode;
  children?: ReactNode;
  className?: string;
}

type SettingsTone = 'neutral' | 'info' | 'warning' | 'danger';

const insetToneClasses: Record<SettingsTone, string> = {
  neutral: 'border-gray-200 bg-gray-50',
  info: 'border-blue-200 bg-blue-50 text-blue-800',
  warning: 'border-amber-200 bg-amber-50 text-amber-800',
  danger: 'border-red-200 bg-red-50 text-red-800',
};

export function SettingsPageTitle({
  children,
  className,
}: SettingsPanelProps) {
  return (
    <h1 className={cn('text-3xl font-bold text-gray-900', className)}>
      {children}
    </h1>
  );
}

export function SettingsTabHeader({
  title,
  description,
  className,
}: SettingsPanelHeaderProps) {
  return (
    <div className={className}>
      <h2 className="text-xl font-semibold text-gray-900">{title}</h2>
      {description && <p className="mt-2 text-sm text-gray-600">{description}</p>}
    </div>
  );
}

export function SettingsPanelTitle({
  children,
  className,
}: SettingsPanelProps) {
  return (
    <h3 className={cn('text-lg font-semibold text-gray-900', className)}>
      {children}
    </h3>
  );
}

export function SettingsSubsectionTitle({
  children,
  className,
}: SettingsPanelProps) {
  return (
    <h4 className={cn('text-sm font-medium text-gray-900', className)}>
      {children}
    </h4>
  );
}

export function SettingsField({
  title,
  description,
  children,
  className,
  contentClassName,
}: SettingsFieldProps) {
  return (
    <div className={cn('space-y-1.5', className)}>
      <div>
        <div className="text-sm font-medium text-gray-700">{title}</div>
        {description && <p className="mt-1 text-xs text-gray-500">{description}</p>}
      </div>
      <div className={cn('flex gap-2', contentClassName)}>{children}</div>
    </div>
  );
}

export function SettingsPanel({ children, className }: SettingsPanelProps) {
  return (
    <section className={cn('rounded-lg border border-gray-200 bg-white p-6 shadow-sm', className)}>
      {children}
    </section>
  );
}

export function SettingsPanelHeader({
  title,
  description,
  className,
}: SettingsPanelHeaderProps) {
  return (
    <div className={className}>
      <SettingsPanelTitle>{title}</SettingsPanelTitle>
      {description && <p className="mt-2 text-sm text-gray-600">{description}</p>}
    </div>
  );
}

export function SettingsTogglePanel({
  title,
  description,
  control,
  children,
  className,
}: SettingsTogglePanelProps) {
  return (
    <SettingsPanel className={className}>
      <div className="flex items-center justify-between gap-6">
        <div className="min-w-0 flex-1">
          <SettingsPanelHeader title={title} description={description} />
        </div>
        <div className="shrink-0">{control}</div>
      </div>
      {children && <div className="mt-4">{children}</div>}
    </SettingsPanel>
  );
}

interface SettingsInsetProps extends SettingsPanelProps {
  tone?: SettingsTone;
}

export function SettingsInset({
  children,
  className,
  tone = 'neutral',
}: SettingsInsetProps) {
  return (
    <div className={cn('rounded-lg border p-4', insetToneClasses[tone], className)}>
      {children}
    </div>
  );
}

export function SettingsNotice({
  children,
  className,
  tone = 'info',
}: SettingsInsetProps) {
  return (
    <div className={cn('rounded-lg border p-4 text-sm', insetToneClasses[tone], className)}>
      {children}
    </div>
  );
}
