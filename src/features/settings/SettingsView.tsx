import { useState } from "react";
import { AlertCircle, CheckCircle, Save } from "lucide-react";
import * as Dialog from "@radix-ui/react-dialog";

import { BurnlyClientError } from "../../ipc/errors";
import type {
  AppCapabilitiesResponse,
  SettingsResponse,
  UpdateSettingsRequest,
} from "../../ipc/generated/contracts";
import {
  useSettings,
  useUpdateProjectPathRetention,
  useUpdateSettings,
} from "./use-settings";

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
  const privacyMutation = useUpdateProjectPathRetention();

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
      privacySaving={privacyMutation.isPending}
      privacyError={privacyMutation.error}
      clearedPaths={privacyMutation.data?.clearedPaths}
      onPrivacyChange={(retainPaths) => {
        privacyMutation.reset();
        privacyMutation.mutate({
          expectedRevision: settingsQuery.data.revision,
          retainPaths,
        });
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
  privacySaving: boolean;
  privacyError: Error | null;
  clearedPaths: number | undefined;
  onPrivacyChange: (retainPaths: boolean) => void;
}

function SettingsForm({
  settings,
  capabilities,
  saving,
  error,
  saved,
  onSave,
  onReload,
  privacySaving,
  privacyError,
  clearedPaths,
  onPrivacyChange,
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
          onSave(settingsRequest(settings, form));
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
        <PlatformSettings
          settings={settings}
          capabilities={capabilities}
          notificationsEnabled={form.notificationsEnabled}
          onNotificationsChange={form.setNotificationsEnabled}
          privacySaving={privacySaving}
          privacyError={privacyError}
          clearedPaths={clearedPaths}
          onPrivacyChange={onPrivacyChange}
        />
        <SubmitSettings saving={saving} />
      </form>
    </div>
  );
}

function settingsRequest(
  settings: SettingsResponse,
  form: ReturnType<typeof useSettingsFormState>,
): UpdateSettingsRequest {
  return {
    expectedRevision: settings.revision,
    reportingTimezone: form.reportingTimezone,
    backgroundRefreshEnabled: form.backgroundRefreshEnabled,
    refreshIntervalMinutes: form.refreshIntervalMinutes,
    launchAtLogin: settings.launchAtLogin,
    closeBehavior: form.closeBehavior,
    notificationsEnabled: form.notificationsEnabled,
    storeProjectPaths: settings.storeProjectPaths,
  };
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
  const [notificationsEnabled, setNotificationsEnabled] = useState(
    settings.notificationsEnabled,
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
    notificationsEnabled,
    setNotificationsEnabled,
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
  notificationsEnabled,
  onNotificationsChange,
  privacySaving,
  privacyError,
  clearedPaths,
  onPrivacyChange,
}: {
  settings: SettingsResponse;
  capabilities: AppCapabilitiesResponse;
  notificationsEnabled: boolean;
  onNotificationsChange: (enabled: boolean) => void;
  privacySaving: boolean;
  privacyError: Error | null;
  clearedPaths: number | undefined;
  onPrivacyChange: (retainPaths: boolean) => void;
}) {
  return (
    <section className="space-y-4 border-t border-zinc-800 pt-6">
      <ReadOnlySetting
        label="Launch at login"
        checked={settings.launchAtLogin}
        available={capabilities.launchAtLogin.supported}
      />
      {capabilities.nativeNotifications.supported ? (
        <div className="space-y-1">
          <Toggle
            id="native-notifications"
            label="Native notifications"
            checked={notificationsEnabled}
            onChange={onNotificationsChange}
          />
          <p className="text-xs text-zinc-500">
            Permission: {capabilities.nativeNotifications.permission}
          </p>
        </div>
      ) : (
        <ReadOnlySetting
          label="Native notifications"
          checked={false}
          available={false}
        />
      )}
      <ProjectPathPrivacy
        enabled={settings.storeProjectPaths}
        saving={privacySaving}
        error={privacyError}
        clearedPaths={clearedPaths}
        onChange={onPrivacyChange}
      />
    </section>
  );
}

function ProjectPathPrivacy({
  enabled,
  saving,
  error,
  clearedPaths,
  onChange,
}: {
  enabled: boolean;
  saving: boolean;
  error: Error | null;
  clearedPaths: number | undefined;
  onChange: (enabled: boolean) => void;
}) {
  return enabled ? (
    <DisableProjectPaths saving={saving} onChange={onChange} />
  ) : (
    <EnableProjectPaths
      saving={saving}
      error={error}
      clearedPaths={clearedPaths}
      onChange={onChange}
    />
  );
}

function EnableProjectPaths({
  saving,
  error,
  clearedPaths,
  onChange,
}: Omit<Parameters<typeof ProjectPathPrivacy>[0], "enabled">) {
  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between gap-4">
        <span className="text-sm text-zinc-300">Store project paths</span>
        <button
          type="button"
          disabled={saving}
          className="border border-zinc-700 px-3 py-1.5 text-xs text-zinc-200 hover:border-zinc-500 disabled:opacity-50"
          onClick={() => {
            onChange(true);
          }}
        >
          Enable
        </button>
      </div>
      {clearedPaths !== undefined ? (
        <p className="text-xs text-zinc-500">
          Removed {clearedPaths} stored project paths.
        </p>
      ) : null}
      {error ? (
        <p className="text-xs text-red-400">{errorMessage(error)}</p>
      ) : null}
    </div>
  );
}

function DisableProjectPaths({
  saving,
  onChange,
}: Pick<Parameters<typeof ProjectPathPrivacy>[0], "saving" | "onChange">) {
  return (
    <Dialog.Root>
      <div className="flex items-center justify-between gap-4">
        <span className="text-sm text-zinc-300">Store project paths</span>
        <Dialog.Trigger asChild>
          <button
            type="button"
            disabled={saving}
            className="border border-red-900 px-3 py-1.5 text-xs text-red-300 hover:border-red-700 disabled:opacity-50"
          >
            Disable
          </button>
        </Dialog.Trigger>
      </div>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/70" />
        <Dialog.Content className="fixed left-1/2 top-1/2 w-[min(28rem,calc(100vw-2rem))] -translate-x-1/2 -translate-y-1/2 border border-zinc-700 bg-zinc-950 p-6 text-zinc-100">
          <Dialog.Title className="text-lg font-semibold">
            Remove stored project paths?
          </Dialog.Title>
          <Dialog.Description className="mt-2 text-sm text-zinc-400">
            Burnly will permanently remove stored workspace paths. Usage history
            and private project grouping identifiers will remain.
          </Dialog.Description>
          <div className="mt-6 flex justify-end gap-3">
            <Dialog.Close asChild>
              <button
                type="button"
                className="border border-zinc-700 px-3 py-2 text-sm"
              >
                Cancel
              </button>
            </Dialog.Close>
            <Dialog.Close asChild>
              <button
                type="button"
                className="bg-red-700 px-3 py-2 text-sm text-white hover:bg-red-600"
                onClick={() => {
                  onChange(false);
                }}
              >
                Remove paths
              </button>
            </Dialog.Close>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
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
