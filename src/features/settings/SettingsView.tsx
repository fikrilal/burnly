import { useState } from "react";
import { Save, CheckCircle, AlertCircle } from "lucide-react";
import { updateSettings } from "../../ipc/client";
import type { AppBootstrapResponse } from "../../ipc/generated/contracts";

interface SettingsViewProps {
  settings: AppBootstrapResponse["settings"];
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

export function SettingsView({ settings }: SettingsViewProps) {
  const [reportingTimezone, setReportingTimezone] = useState(
    settings.reportingTimezone,
  );
  const [backgroundRefreshEnabled, setBackgroundRefreshEnabled] = useState(
    settings.backgroundRefreshEnabled,
  );
  const [refreshIntervalMinutes, setRefreshIntervalMinutes] = useState(
    settings.refreshIntervalMinutes,
  );
  const [launchAtLogin, setLaunchAtLogin] = useState(settings.launchAtLogin);
  const [closeBehavior, setCloseBehavior] = useState<"hide" | "quit">(
    settings.closeBehavior,
  );
  const [notificationsEnabled, setNotificationsEnabled] = useState(
    settings.notificationsEnabled,
  );
  const [storeProjectPaths, setStoreProjectPaths] = useState(
    settings.storeProjectPaths,
  );

  const [saving, setSaving] = useState(false);
  const [saveStatus, setSaveStatus] = useState<{
    type: "success" | "error";
    message: string;
  } | null>(null);

  const handleSave = async (e: React.SyntheticEvent) => {
    e.preventDefault();
    setSaving(true);
    setSaveStatus(null);

    try {
      await updateSettings({
        reportingTimezone,
        backgroundRefreshEnabled,
        refreshIntervalMinutes,
        launchAtLogin,
        closeBehavior,
        notificationsEnabled,
        storeProjectPaths,
      });
      setSaveStatus({
        type: "success",
        message: "Settings saved successfully.",
      });
    } catch (err) {
      setSaveStatus({
        type: "error",
        message:
          err instanceof Error ? err.message : "Failed to save settings.",
      });
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="max-w-2xl mx-auto rounded-lg border border-zinc-800 bg-zinc-900/50 p-6 md:p-8">
      <div className="border-b border-zinc-800 pb-4 mb-6">
        <h2 className="text-xl font-semibold text-white">Settings</h2>
        <p className="text-sm text-zinc-400 mt-1">
          Configure reporting, performance, and application preferences.
        </p>
      </div>

      <form
        onSubmit={(e) => {
          void handleSave(e);
        }}
        className="space-y-6"
      >
        {saveStatus && (
          <div
            className={`flex items-start gap-2 rounded border p-4 text-sm ${
              saveStatus.type === "success"
                ? "border-emerald-900/30 bg-emerald-950/20 text-emerald-400"
                : "border-red-900/30 bg-red-950/20 text-red-400"
            }`}
          >
            {saveStatus.type === "success" ? (
              <CheckCircle className="h-5 w-5 shrink-0 mt-0.5" />
            ) : (
              <AlertCircle className="h-5 w-5 shrink-0 mt-0.5" />
            )}
            <p>{saveStatus.message}</p>
          </div>
        )}

        {/* Timezone Setting */}
        <div className="space-y-2">
          <label
            htmlFor="timezone-select"
            className="block text-sm font-medium text-zinc-300"
          >
            Reporting Timezone
          </label>
          <div className="flex gap-2">
            <select
              id="timezone-select"
              value={
                COMMON_TIMEZONES.includes(reportingTimezone)
                  ? reportingTimezone
                  : "custom"
              }
              onChange={(e) => {
                if (e.target.value !== "custom") {
                  setReportingTimezone(e.target.value);
                }
              }}
              className="rounded-md border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-white focus:outline-none focus:ring-1 focus:ring-cyan-500 w-1/2"
            >
              {COMMON_TIMEZONES.map((tz) => (
                <option key={tz} value={tz}>
                  {tz}
                </option>
              ))}
              <option value="custom">Custom timezone...</option>
            </select>

            <input
              type="text"
              aria-label="Custom Timezone"
              placeholder="e.g. UTC"
              value={reportingTimezone}
              onChange={(e) => {
                setReportingTimezone(e.target.value);
              }}
              className="rounded-md border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-white focus:outline-none focus:ring-1 focus:ring-cyan-500 w-1/2"
            />
          </div>
          <p className="text-xs text-zinc-500">
            Dates and periods will be grouped using this timezone.
          </p>
        </div>

        {/* Close Behavior */}
        <div className="space-y-2">
          <label
            htmlFor="close-behavior"
            className="block text-sm font-medium text-zinc-300"
          >
            Close Behavior
          </label>
          <select
            id="close-behavior"
            value={closeBehavior}
            onChange={(e) => {
              const val = e.target.value;
              if (val === "hide" || val === "quit") {
                setCloseBehavior(val);
              }
            }}
            className="rounded-md border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-white focus:outline-none focus:ring-1 focus:ring-cyan-500 w-full"
          >
            <option value="quit">Quit Application</option>
            <option value="hide">Hide to System Tray</option>
          </select>
          <p className="text-xs text-zinc-500">
            Choose whether to keep the app active in the background when closing
            the main window.
          </p>
        </div>

        {/* Background Sync */}
        <div className="border-t border-zinc-800 pt-6 space-y-4">
          <div className="flex items-start gap-3">
            <input
              id="bg-refresh"
              type="checkbox"
              checked={backgroundRefreshEnabled}
              onChange={(e) => {
                setBackgroundRefreshEnabled(e.target.checked);
              }}
              className="mt-1 h-4 w-4 rounded border-zinc-800 bg-zinc-950 text-cyan-500 focus:ring-cyan-500 focus:ring-offset-zinc-900 focus:ring-1"
            />
            <div className="space-y-1">
              <label
                htmlFor="bg-refresh"
                className="text-sm font-medium text-zinc-300"
              >
                Enable Background Sync
              </label>
              <p className="text-xs text-zinc-500">
                Periodically import usage data from collectors in the
                background.
              </p>
            </div>
          </div>

          {backgroundRefreshEnabled && (
            <div className="pl-7 space-y-2">
              <label
                htmlFor="refresh-interval"
                className="block text-sm font-medium text-zinc-300"
              >
                Interval (minutes)
              </label>
              <input
                id="refresh-interval"
                type="number"
                min="5"
                value={refreshIntervalMinutes}
                onChange={(e) => {
                  setRefreshIntervalMinutes(Number(e.target.value));
                }}
                className="rounded-md border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-white focus:outline-none focus:ring-1 focus:ring-cyan-500 w-32"
              />
            </div>
          )}
        </div>

        {/* Application Behavior Toggles */}
        <div className="border-t border-zinc-800 pt-6 space-y-4">
          <h3 className="text-sm font-semibold text-zinc-400 uppercase tracking-wider">
            General Behavior
          </h3>

          <div className="flex items-start gap-3">
            <input
              id="launch-login"
              type="checkbox"
              checked={launchAtLogin}
              onChange={(e) => {
                setLaunchAtLogin(e.target.checked);
              }}
              className="mt-1 h-4 w-4 rounded border-zinc-800 bg-zinc-950 text-cyan-500 focus:ring-cyan-500 focus:ring-offset-zinc-900 focus:ring-1"
            />
            <div className="space-y-1">
              <label
                htmlFor="launch-login"
                className="text-sm font-medium text-zinc-300"
              >
                Launch at Login
              </label>
              <p className="text-xs text-zinc-500">
                Automatically start Burnly when you log in to your computer.
              </p>
            </div>
          </div>

          <div className="flex items-start gap-3">
            <input
              id="notifications"
              type="checkbox"
              checked={notificationsEnabled}
              onChange={(e) => {
                setNotificationsEnabled(e.target.checked);
              }}
              className="mt-1 h-4 w-4 rounded border-zinc-800 bg-zinc-950 text-cyan-500 focus:ring-cyan-500 focus:ring-offset-zinc-900 focus:ring-1"
            />
            <div className="space-y-1">
              <label
                htmlFor="notifications"
                className="text-sm font-medium text-zinc-300"
              >
                Native Notifications
              </label>
              <p className="text-xs text-zinc-500">
                Show desktop notifications for budget thresholds and alerts.
              </p>
            </div>
          </div>

          <div className="flex items-start gap-3">
            <input
              id="project-paths"
              type="checkbox"
              checked={storeProjectPaths}
              onChange={(e) => {
                setStoreProjectPaths(e.target.checked);
              }}
              className="mt-1 h-4 w-4 rounded border-zinc-800 bg-zinc-950 text-cyan-500 focus:ring-cyan-500 focus:ring-offset-zinc-900 focus:ring-1"
            />
            <div className="space-y-1">
              <label
                htmlFor="project-paths"
                className="text-sm font-medium text-zinc-300"
              >
                Store Project Paths
              </label>
              <p className="text-xs text-zinc-500">
                Keep the local workspace folder paths associated with your token
                usage. Turning this off redacts paths for privacy.
              </p>
            </div>
          </div>
        </div>

        {/* Submit */}
        <div className="border-t border-zinc-800 pt-6 flex justify-end">
          <button
            type="submit"
            disabled={saving}
            className="inline-flex items-center justify-center gap-2 rounded-md bg-cyan-600 hover:bg-cyan-500 active:bg-cyan-700 px-5 py-2.5 text-sm font-medium text-white transition-colors focus:outline-none focus:ring-2 focus:ring-cyan-500 focus:ring-offset-zinc-950 disabled:opacity-50"
          >
            <Save className="h-4 w-4" />
            {saving ? "Saving..." : "Save Settings"}
          </button>
        </div>
      </form>
    </div>
  );
}
