import { useEffect, useState } from "react";
import { RefreshCw, X } from "lucide-react";

import { hideTrayPanel } from "../../ipc/client";
import type {
  AppCapabilitiesResponse,
  SettingsResponse,
  TraySummaryResponse,
} from "../../ipc/generated/contracts";
import {
  AllocationList,
  CompactMetric,
  EmptyState,
  ErrorState,
  MetricRow,
  type FreshnessState,
  type ModelUsage,
} from "../../components/burnly";
import { AnimatedNumber } from "../../components/ui/animated-number";
import { MotionTabs } from "../../components/ui/motion-tabs";
import { Switch } from "../../components/ui/switch";
import { ThemeToggle } from "../../components/ui/theme-toggle";
import { cn } from "../../lib/cn";
import { formatCompactNumber, formatNumber } from "../../lib/format";
import { useSettings, useUpdateSettings } from "../settings/use-settings";
import { useTraySummary } from "./use-tray-summary";

interface TrayPanelProps {
  reportingTimezone: string;
  appVersion: string;
  capabilities: AppCapabilitiesResponse;
}

export function TrayPanel({
  reportingTimezone,
  appVersion,
  capabilities,
}: TrayPanelProps) {
  const summary = useTraySummary(reportingTimezone);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        void hideTrayPanel();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
    };
  }, []);

  if (summary.isPending) {
    return <TrayShell status="Loading" detail="Reading local usage data" />;
  }

  if (summary.isError && !summary.data) {
    return (
      <TrayShell
        status="Failed"
        detail={userSafeErrorMessage(summary.error)}
        tone="danger"
      />
    );
  }

  return (
    <TrayPanelContent
      summary={summary.data}
      isRefreshing={summary.isRefreshing}
      isError={summary.isError}
      error={summary.error}
      appVersion={appVersion}
      capabilities={capabilities}
    />
  );
}

export function TrayStartupState({
  status,
  detail,
}: {
  status: string;
  detail: string;
}) {
  return <TrayShell status={status} detail={detail} />;
}

function TrayPanelContent({
  summary,
  isRefreshing,
  isError,
  error,
  appVersion,
  capabilities,
}: {
  summary: TraySummaryResponse;
  isRefreshing: boolean;
  isError: boolean;
  error: Error | null;
  appVersion: string;
  capabilities: AppCapabilitiesResponse;
}) {
  const [activeTab, setActiveTab] = useState<string>("overview");

  return (
    <main className="flex min-h-screen flex-col overflow-hidden rounded-2xl border border-border bg-background text-foreground">
      <div className="flex flex-1 flex-col gap-6 p-5">
        <header
          data-tauri-drag-region
          className="flex items-start justify-between gap-3"
        >
          <div className="flex flex-col gap-2">
            <MotionTabs
              tabs={[
                { id: "overview", label: "Overview" },
                { id: "settings", label: "Settings" },
              ]}
              activeTab={activeTab}
              onTabChange={setActiveTab}
            />
            <div className="mt-0.5">
              <HeaderStatus
                state={freshnessState(
                  summary.dataStatus,
                  isRefreshing,
                  isError,
                )}
                updatedAt={summary.lastSuccessfulRefreshAt}
              />
            </div>
          </div>
          <PanelCloseButton />
        </header>

        {activeTab === "overview" ? (
          <OverviewTab summary={summary} isError={isError} error={error} />
        ) : (
          <SettingsTab
            appVersion={appVersion}
            launchAtLoginCapability={capabilities.launchAtLogin}
          />
        )}
      </div>
    </main>
  );
}

function OverviewTab({
  summary,
  isError,
  error,
}: {
  summary: TraySummaryResponse;
  isError: boolean;
  error: Error | null;
}) {
  const isEmpty = summary.dataStatus === "empty";

  return (
    <div className="flex flex-col gap-6">
      {isError ? (
        <ErrorState
          title="Update failed"
          description={userSafeErrorMessage(error)}
        />
      ) : null}

      <CompactMetric
        label="Today token usage"
        value={
          <AnimatedNumber value={tokenNumber(summary.today.totalTokens)} />
        }
        caption="tokens today"
      />

      <MetricRow
        items={[
          {
            label: "This week",
            value: formatCompactNumber(summary.week.totalTokens),
          },
          {
            label: "This month",
            value: formatCompactNumber(summary.month.totalTokens),
          },
        ]}
      />

      {isEmpty ? (
        <EmptyState
          title="No usage collected today"
          description="Burnly updates automatically when data becomes stale."
        />
      ) : null}

      <AllocationList models={toModelUsage(summary.models)} />
    </div>
  );
}

function SettingsTab({
  appVersion,
  launchAtLoginCapability,
}: {
  appVersion: string;
  launchAtLoginCapability: AppCapabilitiesResponse["launchAtLogin"];
}) {
  const settings = useSettings();
  const updateSettings = useUpdateSettings();

  if (settings.isPending) {
    return <SettingsLoading />;
  }

  if (settings.isError) {
    return (
      <SettingsLoadError
        error={settings.error}
        onRetry={() => {
          void settings.refetch();
        }}
      />
    );
  }

  return (
    <SettingsForm
      settings={settings.data}
      appVersion={appVersion}
      launchAtLoginCapability={launchAtLoginCapability}
      isSaving={updateSettings.isPending}
      saveError={updateSettings.error}
      onUpdate={(request) => {
        updateSettings.mutate(request);
      }}
    />
  );
}

function SettingsForm({
  settings,
  appVersion,
  launchAtLoginCapability,
  isSaving,
  saveError,
  onUpdate,
}: {
  settings: SettingsResponse;
  appVersion: string;
  launchAtLoginCapability: AppCapabilitiesResponse["launchAtLogin"];
  isSaving: boolean;
  saveError: Error | null;
  onUpdate: (request: {
    launchAtLogin: boolean;
    closeBehavior: SettingsResponse["closeBehavior"];
    expectedRevision: number;
  }) => void;
}) {
  const changeCloseBehavior = (
    closeBehavior: SettingsResponse["closeBehavior"],
  ) => {
    if (closeBehavior === settings.closeBehavior) return;
    onUpdate({
      launchAtLogin: settings.launchAtLogin,
      closeBehavior,
      expectedRevision: settings.revision,
    });
  };

  const changeLaunchAtLogin = (launchAtLogin: boolean) => {
    if (!launchAtLoginCapability.supported) return;
    if (launchAtLogin === settings.launchAtLogin) return;
    onUpdate({
      launchAtLogin,
      closeBehavior: settings.closeBehavior,
      expectedRevision: settings.revision,
    });
  };

  return (
    <div className="flex flex-1 flex-col justify-between gap-4">
      <div className="flex flex-col">
        <div className="flex flex-col divide-y divide-border">
          <LaunchAtLoginSetting
            value={settings.launchAtLogin}
            isDisabled={isSaving || !launchAtLoginCapability.supported}
            onChange={changeLaunchAtLogin}
          />
          <CloseBehaviorSetting
            value={settings.closeBehavior}
            isSaving={isSaving}
            onChange={changeCloseBehavior}
          />
          <ThemeSetting />
        </div>
        <SettingsSaveError error={saveError} />
      </div>
      <SettingsVersion appVersion={appVersion} />
    </div>
  );
}

function SettingsVersion({ appVersion }: { appVersion: string }) {
  return (
    <div className="text-center text-[10px] font-mono tracking-widest text-muted-foreground/40 uppercase">
      Version {appVersion}
    </div>
  );
}

function SettingsSaveError({ error }: { error: Error | null }) {
  if (!error) return null;

  return (
    <div className="mt-4">
      <ErrorState
        title="Settings not saved"
        description={userSafeErrorMessage(
          error,
          "Burnly could not save settings.",
        )}
      />
    </div>
  );
}

function ThemeSetting() {
  return (
    <div className="flex items-center justify-between gap-4 py-3">
      <div className="flex flex-col gap-1">
        <span className="text-sm font-medium">Theme</span>
        <span className="text-xs text-muted-foreground leading-normal">
          Select the interface color mode.
        </span>
      </div>
      <ThemeToggle />
    </div>
  );
}

function SettingsLoading() {
  return (
    <div className="flex flex-col gap-3">
      <p className="text-sm text-muted-foreground">Loading settings</p>
    </div>
  );
}

function SettingsLoadError({
  error,
  onRetry,
}: {
  error: unknown;
  onRetry: () => void;
}) {
  return (
    <div className="flex flex-col gap-3">
      <ErrorState
        title="Settings unavailable"
        description={userSafeErrorMessage(
          error,
          "Burnly could not load settings.",
        )}
      />
      <button
        type="button"
        onClick={onRetry}
        className="w-fit rounded-md border border-border px-3 py-1.5 text-sm font-medium transition-colors hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
      >
        Retry
      </button>
    </div>
  );
}

function LaunchAtLoginSetting({
  value,
  isDisabled,
  onChange,
}: {
  value: boolean;
  isDisabled: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between gap-4 py-3">
      <div className="flex flex-col gap-1">
        <span className="text-sm font-medium">Launch at login</span>
        <span className="text-xs text-muted-foreground leading-normal">
          Start Burnly automatically when you log into your system.
        </span>
      </div>
      <Switch
        checked={value}
        disabled={isDisabled}
        aria-label="Launch at login"
        onCheckedChange={onChange}
      />
    </div>
  );
}

function CloseBehaviorSetting({
  value,
  isSaving,
  onChange,
}: {
  value: SettingsResponse["closeBehavior"];
  isSaving: boolean;
  onChange: (value: SettingsResponse["closeBehavior"]) => void;
}) {
  const isQuit = value === "quit";

  return (
    <div className="flex items-center justify-between gap-4 py-3">
      <div className="flex flex-col gap-1">
        <span className="text-sm font-medium">Quit on close</span>
        <span className="text-xs text-muted-foreground leading-normal">
          Terminate the application when closing the panel.
        </span>
      </div>
      <div className="flex items-center gap-3">
        <Switch
          checked={isQuit}
          disabled={isSaving}
          aria-label="Quit on close"
          onCheckedChange={(checked) => {
            onChange(checked ? "quit" : "hide");
          }}
        />
      </div>
    </div>
  );
}
function TrayShell({
  status,
  detail,
  tone = "neutral",
}: {
  status: string;
  detail: string;
  tone?: "neutral" | "danger";
}) {
  return (
    <main className="min-h-screen overflow-hidden rounded-2xl border border-border bg-background text-foreground">
      <div className="flex flex-col gap-4 p-5">
        <div
          data-tauri-drag-region
          className="flex items-start justify-between gap-2"
        >
          <span
            className={cn(
              "inline-flex items-center rounded-full px-2.5 py-1 text-xs font-medium",
              tone === "danger"
                ? "bg-destructive/10 text-destructive"
                : "bg-muted text-muted-foreground",
            )}
          >
            {status}
          </span>
          <PanelCloseButton />
        </div>
        <div>
          <h1 className="text-2xl font-semibold">Burnly</h1>
          <p className="mt-1 text-sm text-muted-foreground">{detail}</p>
        </div>
      </div>
    </main>
  );
}

function PanelCloseButton() {
  return (
    <button
      type="button"
      aria-label="Close"
      onClick={() => {
        void hideTrayPanel();
      }}
      className="rounded-md p-1 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
    >
      <X className="size-4" aria-hidden />
    </button>
  );
}

function HeaderStatus({
  state,
  updatedAt,
}: {
  state: FreshnessState;
  updatedAt: string | null;
}) {
  if (state === "failed") {
    return (
      <span className="text-xs font-medium text-destructive">
        Update failed
      </span>
    );
  }
  if (state === "partial") {
    return (
      <span className="text-xs text-muted-foreground">Some sources failed</span>
    );
  }
  if (state === "refreshing") {
    return (
      <span className="inline-flex items-center gap-1.5 text-xs text-muted-foreground">
        <RefreshCw
          className="size-3 animate-spin motion-reduce:animate-none"
          aria-hidden
        />
        Refreshing
      </span>
    );
  }
  return (
    <span className="text-xs text-muted-foreground">
      {relativeUpdated(updatedAt)}
    </span>
  );
}

function relativeUpdated(iso: string | null): string {
  if (!iso) return "Never updated";
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return "Updated recently";
  const minutes = Math.floor((Date.now() - then) / 60000);
  if (minutes < 1) return "Updated just now";
  if (minutes < 60) return `Updated ${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `Updated ${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `Updated ${days}d ago`;
}

function freshnessState(
  dataStatus: TraySummaryResponse["dataStatus"],
  isRefreshing: boolean,
  isError: boolean,
): FreshnessState {
  if (isError) return "failed";
  if (isRefreshing) return "refreshing";
  return dataStatus;
}

// Keep helper functions at end of file.
function toModelUsage(models: TraySummaryResponse["models"]): ModelUsage[] {
  return models.map((model) => ({
    modelName: model.modelName,
    agentLabel: model.agentLabel,
    tokens: formatNumber(model.totalTokens),
    trend: model.trend,
  }));
}

function tokenNumber(value: string): number {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

function userSafeErrorMessage(
  error: unknown,
  fallback = "Burnly could not load tray summary data.",
): string {
  return error instanceof Error ? error.message : fallback;
}
