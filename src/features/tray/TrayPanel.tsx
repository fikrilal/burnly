import {
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
  type RefObject,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { ChevronLeft, ChevronRight, RefreshCw, X } from "lucide-react";

import { hideTrayPanel } from "../../ipc/client";
import type {
  AppCapabilitiesResponse,
  DiagnosticsHealthResponse,
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
import { Button } from "../../components/ui/button";
import { MotionTabs } from "../../components/ui/motion-tabs";
import { Switch } from "../../components/ui/switch";
import { ThemeToggle } from "../../components/ui/theme-toggle";
import { cn } from "../../lib/cn";
import { formatCompactNumber, formatNumber } from "../../lib/format";
import {
  useCopyDiagnosticsReport,
  useDiagnosticsHealth,
  useExportDiagnosticsReport,
} from "../diagnostics/use-diagnostics";
import { useSettings, useUpdateSettings } from "../settings/use-settings";
import { UpdateSetting } from "../update/UpdateSetting";
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
  return (
    <TrayScrollArea label={activeTab === "overview" ? "Overview" : "Settings"}>
      {activeTab === "overview" ? (
        <OverviewTab summary={summary} isError={isError} error={error} />
      ) : (
        <SettingsTab
          appVersion={appVersion}
          launchAtLoginCapability={capabilities.launchAtLogin}
          diagnosticsCapabilities={capabilities.diagnostics}
        />
      )}
    </TrayScrollArea>
  );
}

interface ScrollMetrics {
  visible: boolean;
  thumbHeight: number;
  thumbTop: number;
}

const MIN_SCROLL_THUMB_HEIGHT = 32;
const SCROLLBAR_IDLE_HIDE_MS = 1_600;

function TrayScrollArea({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  const viewportRef = useRef<HTMLElement | null>(null);
  const { metrics, updateMetrics } = useTrayScrollMetrics(viewportRef);
  const { isActive, activate } = useTransientScrollbar(metrics.visible);
  const startDragging = useTrayScrollDrag(
    viewportRef,
    metrics.thumbHeight,
    activate,
  );

  useLayoutEffect(() => {
    updateMetrics();
  }, [children, label, updateMetrics]);

  const onScroll = () => {
    updateMetrics();
    activate();
  };

  return (
    <div className="tray-scroll-shell relative min-h-0 flex-1">
      <section
        ref={viewportRef}
        aria-label={label}
        className="tray-scroll-area h-full overflow-y-auto pr-5"
        onScroll={onScroll}
      >
        {children}
      </section>
      {metrics.visible ? (
        <div
          className="tray-scroll-track"
          data-active={isActive ? "true" : "false"}
          aria-hidden="true"
        >
          <div
            className="tray-scroll-thumb"
            onPointerDown={startDragging}
            style={{
              height: `${metrics.thumbHeight}px`,
              transform: `translateY(${metrics.thumbTop}px)`,
            }}
          />
        </div>
      ) : null}
    </div>
  );
}

function useTrayScrollMetrics(viewportRef: RefObject<HTMLElement | null>) {
  const [metrics, setMetrics] = useState<ScrollMetrics>({
    visible: false,
    thumbHeight: MIN_SCROLL_THUMB_HEIGHT,
    thumbTop: 0,
  });

  const updateMetrics = useCallback(() => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    setMetrics(scrollMetrics(viewport));
  }, [viewportRef]);

  useEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport) return;

    const observer =
      typeof ResizeObserver === "undefined"
        ? null
        : new ResizeObserver(updateMetrics);
    observer?.observe(viewport);
    window.addEventListener("resize", updateMetrics);

    return () => {
      observer?.disconnect();
      window.removeEventListener("resize", updateMetrics);
    };
  }, [updateMetrics, viewportRef]);

  return { metrics, updateMetrics };
}

function useTransientScrollbar(isScrollable: boolean) {
  const idleTimerRef = useRef<number | null>(null);
  const [isActive, setIsActive] = useState(false);

  const clearIdleTimer = useCallback(() => {
    if (idleTimerRef.current === null) return;
    window.clearTimeout(idleTimerRef.current);
    idleTimerRef.current = null;
  }, []);

  const activate = useCallback(() => {
    if (!isScrollable) return;
    clearIdleTimer();
    setIsActive(true);
    idleTimerRef.current = window.setTimeout(() => {
      setIsActive(false);
      idleTimerRef.current = null;
    }, SCROLLBAR_IDLE_HIDE_MS);
  }, [clearIdleTimer, isScrollable]);

  useEffect(() => clearIdleTimer, [clearIdleTimer]);

  return { isActive, activate };
}

function useTrayScrollDrag(
  viewportRef: RefObject<HTMLElement | null>,
  thumbHeight: number,
  activate: () => void,
) {
  const dragStateRef = useRef<{
    pointerOffsetY: number;
    maxScrollTop: number;
    maxThumbTop: number;
  } | null>(null);

  useEffect(() => {
    const onPointerMove = (event: PointerEvent) => {
      updateDraggedScrollTop(viewportRef.current, dragStateRef.current, event);
      if (dragStateRef.current) activate();
    };
    const onPointerUp = () => {
      dragStateRef.current = null;
    };

    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    return () => {
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
    };
  }, [activate, viewportRef]);

  return (event: ReactPointerEvent<HTMLDivElement>) => {
    const viewport = viewportRef.current;
    if (!viewport) return;

    const maxScrollTop = viewport.scrollHeight - viewport.clientHeight;
    const maxThumbTop = viewport.clientHeight - thumbHeight;
    if (maxScrollTop <= 0 || maxThumbTop <= 0) return;

    dragStateRef.current = {
      pointerOffsetY:
        event.clientY - event.currentTarget.getBoundingClientRect().top,
      maxScrollTop,
      maxThumbTop,
    };
    activate();
    event.preventDefault();
  };
}

function scrollMetrics(viewport: HTMLElement): ScrollMetrics {
  const { clientHeight, scrollHeight, scrollTop } = viewport;
  const maxScrollTop = scrollHeight - clientHeight;
  if (clientHeight <= 0 || maxScrollTop <= 0) {
    return {
      visible: false,
      thumbHeight: MIN_SCROLL_THUMB_HEIGHT,
      thumbTop: 0,
    };
  }

  const thumbHeight = Math.max(
    MIN_SCROLL_THUMB_HEIGHT,
    (clientHeight / scrollHeight) * clientHeight,
  );
  return {
    visible: true,
    thumbHeight,
    thumbTop: (scrollTop / maxScrollTop) * (clientHeight - thumbHeight),
  };
}

function updateDraggedScrollTop(
  viewport: HTMLElement | null,
  drag: {
    pointerOffsetY: number;
    maxScrollTop: number;
    maxThumbTop: number;
  } | null,
  event: PointerEvent,
) {
  if (!viewport || !drag || drag.maxThumbTop <= 0) return;

  const trackTop = viewport.getBoundingClientRect().top;
  const requestedThumbTop = event.clientY - trackTop - drag.pointerOffsetY;
  const boundedThumbTop = Math.max(
    0,
    Math.min(drag.maxThumbTop, requestedThumbTop),
  );
  viewport.scrollTop = (boundedThumbTop / drag.maxThumbTop) * drag.maxScrollTop;
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
          <AnimatedNumber
            value={tokenNumber(summary.today.totalTokens)}
            format={formatNumber}
          />
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
  diagnosticsCapabilities,
}: {
  appVersion: string;
  launchAtLoginCapability: AppCapabilitiesResponse["launchAtLogin"];
  diagnosticsCapabilities: AppCapabilitiesResponse["diagnostics"];
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
      diagnosticsCapabilities={diagnosticsCapabilities}
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
  diagnosticsCapabilities,
  isSaving,
  saveError,
  onUpdate,
}: {
  settings: SettingsResponse;
  appVersion: string;
  launchAtLoginCapability: AppCapabilitiesResponse["launchAtLogin"];
  diagnosticsCapabilities: AppCapabilitiesResponse["diagnostics"];
  isSaving: boolean;
  saveError: Error | null;
  onUpdate: (request: {
    launchAtLogin: boolean;
    closeBehavior: SettingsResponse["closeBehavior"];
    expectedRevision: number;
  }) => void;
}) {
  const [settingsPage, setSettingsPage] = useState<"list" | "diagnostics">(
    "list",
  );
  const { changeCloseBehavior, changeLaunchAtLogin } = useSettingsFormActions({
    settings,
    launchAtLoginCapability,
    onUpdate,
  });

  if (settingsPage === "diagnostics") {
    return (
      <SettingsPageShell appVersion={appVersion}>
        <DiagnosticsPage
          capabilities={diagnosticsCapabilities}
          onBack={() => {
            setSettingsPage("list");
          }}
        />
      </SettingsPageShell>
    );
  }

  return (
    <SettingsPageShell appVersion={appVersion}>
      <div className="flex flex-col">
        <SettingsList
          settings={settings}
          launchAtLoginCapability={launchAtLoginCapability}
          isSaving={isSaving}
          onChangeLaunchAtLogin={changeLaunchAtLogin}
          onChangeCloseBehavior={changeCloseBehavior}
          onOpenDiagnostics={() => {
            setSettingsPage("diagnostics");
          }}
        />
        <SettingsSaveError error={saveError} />
      </div>
    </SettingsPageShell>
  );
}

function useSettingsFormActions({
  settings,
  launchAtLoginCapability,
  onUpdate,
}: {
  settings: SettingsResponse;
  launchAtLoginCapability: AppCapabilitiesResponse["launchAtLogin"];
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

  return { changeCloseBehavior, changeLaunchAtLogin };
}

function SettingsPageShell({
  appVersion,
  children,
}: {
  appVersion: string;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-1 flex-col justify-between gap-4">
      {children}
      <SettingsVersion appVersion={appVersion} />
    </div>
  );
}

function SettingsList({
  settings,
  launchAtLoginCapability,
  isSaving,
  onChangeLaunchAtLogin,
  onChangeCloseBehavior,
  onOpenDiagnostics,
}: {
  settings: SettingsResponse;
  launchAtLoginCapability: AppCapabilitiesResponse["launchAtLogin"];
  isSaving: boolean;
  onChangeLaunchAtLogin: (value: boolean) => void;
  onChangeCloseBehavior: (value: SettingsResponse["closeBehavior"]) => void;
  onOpenDiagnostics: () => void;
}) {
  return (
    <div className="flex flex-col divide-y divide-border">
      <LaunchAtLoginSetting
        value={settings.launchAtLogin}
        isDisabled={isSaving || !launchAtLoginCapability.supported}
        onChange={onChangeLaunchAtLogin}
      />
      <CloseBehaviorSetting
        value={settings.closeBehavior}
        isSaving={isSaving}
        onChange={onChangeCloseBehavior}
      />
      <ThemeSetting />
      <UpdateSetting />
      <DiagnosticsEntrySetting onOpen={onOpenDiagnostics} />
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

function DiagnosticsEntrySetting({ onOpen }: { onOpen: () => void }) {
  return (
    <button
      type="button"
      aria-label="Open diagnostics"
      className="flex items-center justify-between gap-4 py-3 text-left transition-colors hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
      onClick={onOpen}
    >
      <div className="flex flex-col gap-1">
        <span className="text-sm font-medium">Diagnostics</span>
        <span className="text-xs text-muted-foreground leading-normal">
          View app health and export a local report.
        </span>
      </div>
      <ChevronRight className="size-4 shrink-0 text-muted-foreground" />
    </button>
  );
}

function DiagnosticsPage({
  capabilities,
  onBack,
}: {
  capabilities: AppCapabilitiesResponse["diagnostics"];
  onBack: () => void;
}) {
  const health = useDiagnosticsHealth();
  const exportReport = useExportDiagnosticsReport();
  const copyReport = useCopyDiagnosticsReport();
  const helper = getDiagnosticsHelper(health.data);
  const mutationError = exportReport.error ?? copyReport.error;
  const isMutating = exportReport.isPending || copyReport.isPending;

  return (
    <div className="flex flex-col gap-4 py-1">
      <button
        type="button"
        className="flex w-fit items-center gap-1 rounded-md py-1 pr-2 text-sm font-medium text-muted-foreground transition-colors hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
        onClick={onBack}
      >
        <ChevronLeft className="size-4" />
        <span>Settings</span>
      </button>
      <div className="flex flex-col gap-1">
        <span className="text-base font-semibold">Diagnostics</span>
        <span className="text-xs text-muted-foreground leading-normal">
          Export or copy a local report when support asks for details.
        </span>
      </div>
      <DiagnosticsSummary helper={helper} />
      <DiagnosticsActions
        isBusy={isMutating || health.isPending}
        canSend={capabilities.sendReport.supported}
        onExport={() => {
          copyReport.reset();
          exportReport.mutate();
        }}
        onCopy={() => {
          exportReport.reset();
          copyReport.mutate();
        }}
      />
      <DiagnosticsActionStatus
        exportStatus={exportReport.data?.status}
        copyStatus={copyReport.data?.status}
      />
      {mutationError ? <DiagnosticsActionError error={mutationError} /> : null}
    </div>
  );
}

function DiagnosticsSummary({
  helper,
}: {
  helper: { message: string; className: string };
}) {
  return (
    <span className={cn("text-xs leading-normal", helper.className)}>
      {helper.message}
    </span>
  );
}

function DiagnosticsActions({
  isBusy,
  canSend,
  onExport,
  onCopy,
}: {
  isBusy: boolean;
  canSend: boolean;
  onExport: () => void;
  onCopy: () => void;
}) {
  return (
    <div className="flex shrink-0 items-center gap-2">
      <Button
        type="button"
        variant="outline"
        size="xs"
        disabled={isBusy}
        onClick={onExport}
      >
        <span className="text-xs leading-none">Export</span>
      </Button>
      <Button
        type="button"
        variant="outline"
        size="xs"
        disabled={isBusy}
        onClick={onCopy}
      >
        <span className="text-xs leading-none">Copy</span>
      </Button>
      <Button
        type="button"
        variant="outline"
        size="xs"
        disabled
        title={
          canSend
            ? "Report sending needs a backend endpoint."
            : "Report sending is coming later."
        }
      >
        <span className="text-xs leading-none">Send</span>
      </Button>
    </div>
  );
}

function DiagnosticsActionStatus({
  exportStatus,
  copyStatus,
}: {
  exportStatus: "exported" | "cancelled" | undefined;
  copyStatus: "copied" | undefined;
}) {
  const statusText =
    exportStatus === "exported"
      ? "Diagnostics report exported."
      : copyStatus === "copied"
        ? "Diagnostics report copied."
        : null;

  if (!statusText) return null;

  return <span className="text-xs text-muted-foreground">{statusText}</span>;
}

function DiagnosticsActionError({ error }: { error: unknown }) {
  return (
    <ErrorState
      title="Diagnostics failed"
      description={userSafeErrorMessage(
        error,
        "Burnly could not create the diagnostics report.",
      )}
    />
  );
}

function getDiagnosticsHelper(health: DiagnosticsHealthResponse | undefined): {
  message: string;
  className: string;
} {
  if (!health) {
    return {
      message: "Checking diagnostics status...",
      className: "text-muted-foreground",
    };
  }

  if (health.status === "error") {
    return {
      message:
        "Burnly detected an error. Export diagnostics to help troubleshoot it.",
      className: "text-destructive",
    };
  }

  if (health.status === "warning") {
    return {
      message:
        "Burnly detected a problem. Export diagnostics if support asks for details.",
      className: "text-muted-foreground",
    };
  }

  return {
    message: "No problems detected.",
    className: "text-muted-foreground",
  };
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
