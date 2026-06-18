import { useState } from "react";
import { AlertCircle, CheckCircle, Save } from "lucide-react";

import { BurnlyClientError } from "../../ipc/errors";
import type {
  AppCapabilitiesResponse,
  SettingsResponse,
  UpdateSettingsRequest,
} from "../../ipc/generated/contracts";
import { useSettings, useUpdateSettings } from "./use-settings";

interface SettingsViewProps {
  capabilities: AppCapabilitiesResponse;
}

const COMMON_TIMEZONES = [
  "UTC",
  "Asia/Jakarta",
  "Asia/Singapore",
  "Asia/Tokyo",
  "America/New_York",
  "America/Los_Angeles",
  "Europe/London",
  "Europe/Paris",
];

export function SettingsView({ capabilities }: SettingsViewProps) {
  const settingsQuery = useSettings();
  const updateMutation = useUpdateSettings();

  if (settingsQuery.isPending) {
    return <SettingsStatus title="Loading settings" />;
  }
  if (settingsQuery.isError) {
    return (
      <SettingsStatus
        title="Settings unavailable"
        detail={errorMessage(settingsQuery.error)}
      />
    );
  }

  return (
    <SettingsForm
      key={settingsQuery.data.revision}
      settings={settingsQuery.data}
      capabilities={capabilities}
      saving={updateMutation.isPending}
      error={updateMutation.error}
      saved={updateMutation.isSuccess}
      onSave={(request) => {
        updateMutation.reset();
        updateMutation.mutate(request);
      }}
      onReload={() => {
        updateMutation.reset();
        void settingsQuery.refetch();
      }}
    />
  );
}

interface SettingsFormProps {
  settings: SettingsResponse;
  capabilities: AppCapabilitiesResponse;
  saving: boolean;
  error: Error | null;
  saved: boolean;
  onSave: (request: UpdateSettingsRequest) => void;
  onReload: () => void;
}

function SettingsForm({
  settings,
  capabilities,
  saving,
  error,
  saved,
  onSave,
  onReload,
}: SettingsFormProps) {
  const form = useSettingsFormState(settings);

  const isConflict =
    error instanceof BurnlyClientError && error.category === "conflict";

  return (
    <div className="mx-auto max-w-2xl border border-zinc-800 bg-zinc-900/50 p-6 md:p-8">
      <SettingsHeader />
      <form
        onSubmit={(event) => {
          event.preventDefault();
          onSave({
            expectedRevision: settings.revision,
            reportingTimezone: form.reportingTimezone,
            backgroundRefreshEnabled: form.backgroundRefreshEnabled,
            refreshIntervalMinutes: form.refreshIntervalMinutes,
            launchAtLogin: settings.launchAtLogin,
            closeBehavior: form.closeBehavior,
            notificationsEnabled: settings.notificationsEnabled,
            storeProjectPaths: settings.storeProjectPaths,
          });
        }}
        className="space-y-6"
      >
        <FormStatus
          saved={saved}
          error={error}
          isConflict={isConflict}
          onReload={onReload}
        />
        <ReportingSettings
          reportingTimezone={form.reportingTimezone}
          closeBehavior={form.closeBehavior}
          onTimezoneChange={form.setReportingTimezone}
          onCloseBehaviorChange={form.setCloseBehavior}
        />
        <RefreshSettings
          enabled={form.backgroundRefreshEnabled}
          intervalMinutes={form.refreshIntervalMinutes}
          onEnabledChange={form.setBackgroundRefreshEnabled}
          onIntervalChange={form.setRefreshIntervalMinutes}
        />
        <PlatformSettings settings={settings} capabilities={capabilities} />
        <SubmitSettings saving={saving} />
      </form>
    </div>
  );
}

function useSettingsFormState(settings: SettingsResponse) {
  const [reportingTimezone, setReportingTimezone] = useState(
    settings.reportingTimezone,
  );
  const [backgroundRefreshEnabled, setBackgroundRefreshEnabled] = useState(
    settings.backgroundRefreshEnabled,
  );
  const [refreshIntervalMinutes, setRefreshIntervalMinutes] = useState(
    settings.refreshIntervalMinutes,
  );
  const [closeBehavior, setCloseBehavior] = useState<"hide" | "quit">(
    settings.closeBehavior,
  );
  return {
    reportingTimezone,
    setReportingTimezone,
    backgroundRefreshEnabled,
    setBackgroundRefreshEnabled,
    refreshIntervalMinutes,
    setRefreshIntervalMinutes,
    closeBehavior,
    setCloseBehavior,
  };
}

function SettingsHeader() {
  return (
    <div className="mb-6 border-b border-zinc-800 pb-4">
      <h2 className="text-xl font-semibold text-white">Settings</h2>
    </div>
  );
}

function FormStatus({
  saved,
  error,
  isConflict,
  onReload,
}: {
  saved: boolean;
  error: Error | null;
  isConflict: boolean;
  onReload: () => void;
}) {
  if (error) {
    return (
      <StatusMessage icon={AlertCircle} tone="error">
        {errorMessage(error)}
        {isConflict ? (
          <button type="button" className="ml-2 underline" onClick={onReload}>
            Reload
          </button>
        ) : null}
      </StatusMessage>
    );
  }
  return saved ? (
    <StatusMessage icon={CheckCircle} tone="success">
      Settings saved.
    </StatusMessage>
  ) : null;
}

function ReportingSettings({
  reportingTimezone,
  closeBehavior,
  onTimezoneChange,
  onCloseBehaviorChange,
}: {
  reportingTimezone: string;
  closeBehavior: "hide" | "quit";
  onTimezoneChange: (timezone: string) => void;
  onCloseBehaviorChange: (behavior: "hide" | "quit") => void;
}) {
  return (
    <>
      <Field label="Reporting timezone" htmlFor="reporting-timezone">
        <input
          id="reporting-timezone"
          list="common-timezones"
          value={reportingTimezone}
          onChange={(event) => {
            onTimezoneChange(event.target.value);
          }}
          className={inputClass}
        />
        <datalist id="common-timezones">
          {COMMON_TIMEZONES.map((timezone) => (
            <option key={timezone} value={timezone} />
          ))}
        </datalist>
      </Field>
      <Field label="Close behavior" htmlFor="close-behavior">
        <select
          id="close-behavior"
          value={closeBehavior}
          onChange={(event) => {
            const value = event.target.value;
            if (value === "hide" || value === "quit") {
              onCloseBehaviorChange(value);
            }
          }}
          className={inputClass}
        >
          <option value="quit">Quit application</option>
          <option value="hide">Hide to system tray</option>
        </select>
      </Field>
    </>
  );
}

function RefreshSettings({
  enabled,
  intervalMinutes,
  onEnabledChange,
  onIntervalChange,
}: {
  enabled: boolean;
  intervalMinutes: number;
  onEnabledChange: (enabled: boolean) => void;
  onIntervalChange: (minutes: number) => void;
}) {
  return (
    <section className="space-y-4 border-t border-zinc-800 pt-6">
      <Toggle
        id="background-refresh"
        label="Background refresh"
        checked={enabled}
        onChange={onEnabledChange}
      />
      {enabled ? (
        <Field label="Refresh interval (minutes)" htmlFor="refresh-interval">
          <input
            id="refresh-interval"
            type="number"
            min={5}
            max={1440}
            value={intervalMinutes}
            onChange={(event) => {
              onIntervalChange(event.target.valueAsNumber);
            }}
            className={`${inputClass} w-36`}
          />
        </Field>
      ) : null}
    </section>
  );
}

function PlatformSettings({
  settings,
  capabilities,
}: {
  settings: SettingsResponse;
  capabilities: AppCapabilitiesResponse;
}) {
  return (
    <section className="space-y-4 border-t border-zinc-800 pt-6">
      <ReadOnlySetting
        label="Launch at login"
        checked={settings.launchAtLogin}
        available={capabilities.launchAtLogin.supported}
      />
      <ReadOnlySetting
        label="Native notifications"
        checked={settings.notificationsEnabled}
        available={capabilities.nativeNotifications.supported}
      />
      <ReadOnlySetting
        label="Store project paths"
        checked={settings.storeProjectPaths}
        available={false}
      />
    </section>
  );
}

function SubmitSettings({ saving }: { saving: boolean }) {
  return (
    <div className="flex justify-end border-t border-zinc-800 pt-6">
      <button
        type="submit"
        disabled={saving}
        className="inline-flex items-center gap-2 bg-cyan-600 px-5 py-2.5 text-sm font-medium text-white hover:bg-cyan-500 disabled:opacity-50"
      >
        <Save className="h-4 w-4" aria-hidden />
        {saving ? "Saving" : "Save settings"}
      </button>
    </div>
  );
}

function Field({
  label,
  htmlFor,
  children,
}: {
  label: string;
  htmlFor: string;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-2">
      <label
        htmlFor={htmlFor}
        className="block text-sm font-medium text-zinc-300"
      >
        {label}
      </label>
      {children}
    </div>
  );
}

function Toggle({
  id,
  label,
  checked,
  onChange,
}: {
  id: string;
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label htmlFor={id} className="flex items-center justify-between gap-4">
      <span className="text-sm font-medium text-zinc-300">{label}</span>
      <input
        id={id}
        type="checkbox"
        checked={checked}
        onChange={(event) => {
          onChange(event.target.checked);
        }}
        className="h-4 w-4"
      />
    </label>
  );
}

function ReadOnlySetting({
  label,
  checked,
  available,
}: {
  label: string;
  checked: boolean;
  available: boolean;
}) {
  return (
    <div className="flex items-center justify-between gap-4">
      <span className="text-sm text-zinc-300">{label}</span>
      <span className="text-xs text-zinc-500">
        {available ? (checked ? "Enabled" : "Disabled") : "Unavailable"}
      </span>
    </div>
  );
}

function SettingsStatus({ title, detail }: { title: string; detail?: string }) {
  return (
    <div className="mx-auto max-w-2xl border border-zinc-800 p-6">
      <h2 className="text-lg font-semibold">{title}</h2>
      {detail ? <p className="mt-2 text-sm text-zinc-400">{detail}</p> : null}
    </div>
  );
}

function StatusMessage({
  icon: Icon,
  tone,
  children,
}: {
  icon: typeof AlertCircle;
  tone: "success" | "error";
  children: React.ReactNode;
}) {
  return (
    <div
      className={`flex items-center gap-2 border p-3 text-sm ${
        tone === "success"
          ? "border-emerald-900 text-emerald-400"
          : "border-red-900 text-red-400"
      }`}
    >
      <Icon className="h-4 w-4 shrink-0" aria-hidden />
      <span>{children}</span>
    </div>
  );
}

function errorMessage(error: unknown): string {
  return error instanceof Error
    ? error.message
    : "Settings could not be saved.";
}

const inputClass =
  "w-full border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-white focus:outline-none focus:ring-1 focus:ring-cyan-500";
