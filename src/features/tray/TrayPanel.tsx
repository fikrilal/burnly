import { useEffect, useState } from "react";
import { RefreshCw, X } from "lucide-react";

import type { FreshnessState } from "../../components/burnly";
import { MotionTabs } from "../../components/ui/motion-tabs";
import { hideTrayPanel } from "../../ipc/client";
import type {
  AppCapabilitiesResponse,
  TraySummaryResponse,
} from "../../ipc/generated/contracts";
import { cn } from "../../lib/cn";
import { userSafeErrorMessage } from "../../lib/user-safe-error";
import { SettingsTab } from "../settings/SettingsTab";
import { OverviewTab } from "./OverviewTab";
import { TrayScrollArea } from "./TrayScrollArea";
import { freshnessState, relativeUpdated } from "./tray-utils";
import { useTraySummary } from "./use-tray-summary";

interface TrayPanelProps {
  reportingTimezone: string;
  appVersion: string;
  capabilities: AppCapabilitiesResponse;
}

const TRAY_SURFACE_CLASS =
  "tray-surface h-screen overflow-hidden rounded-2xl border border-border bg-background text-foreground";

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
        status="Refresh failed"
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
    <main className={cn(TRAY_SURFACE_CLASS, "flex flex-col")}>
      <div className="flex min-h-0 flex-1 flex-col gap-6 p-5">
        <header
          data-tauri-drag-region="deep"
          className="tray-drag-region flex shrink-0 items-start justify-between gap-3"
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

        <TrayTabContent
          activeTab={activeTab}
          summary={summary}
          isError={isError}
          error={error}
          appVersion={appVersion}
          capabilities={capabilities}
        />
      </div>
    </main>
  );
}

function TrayTabContent({
  activeTab,
  summary,
  isError,
  error,
  appVersion,
  capabilities,
}: {
  activeTab: string;
  summary: TraySummaryResponse;
  isError: boolean;
  error: Error | null;
  appVersion: string;
  capabilities: AppCapabilitiesResponse;
}) {
  const [settingsPage, setSettingsPage] = useState<"list" | "diagnostics">(
    "list",
  );
  const scrollResetKey =
    activeTab === "settings" ? `settings:${settingsPage}` : activeTab;

  return (
    <TrayScrollArea
      label={activeTab === "overview" ? "Overview" : "Settings"}
      resetKey={scrollResetKey}
    >
      {activeTab === "overview" ? (
        <OverviewTab summary={summary} isError={isError} error={error} />
      ) : (
        <SettingsTab
          page={settingsPage}
          appVersion={appVersion}
          launchAtLoginCapability={capabilities.launchAtLogin}
          diagnosticsCapabilities={capabilities.diagnostics}
          onPageChange={setSettingsPage}
        />
      )}
    </TrayScrollArea>
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
    <main className={TRAY_SURFACE_CLASS}>
      <div className="flex flex-col gap-4 p-5">
        <div
          data-tauri-drag-region="deep"
          className="tray-drag-region flex items-start justify-between gap-2"
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
        Refresh failed
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
