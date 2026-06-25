import { useEffect } from "react";
import { X } from "lucide-react";

import { hideTrayPanel, openDetails } from "../../ipc/client";
import type { TraySummaryResponse } from "../../ipc/generated/contracts";
import {
  AllocationList,
  CompactMetric,
  EmptyState,
  ErrorState,
  FreshnessStatus,
  MetricRow,
  OpenDetailsButton,
  type FreshnessState,
  type ModelUsage,
} from "../../components/burnly";
import { AnimatedNumber } from "../../components/ui/animated-number";
import { cn } from "../../lib/cn";
import {
  formatCompactNumber,
  formatDateTime,
  formatNumber,
} from "../../lib/format";
import { useTraySummary } from "./use-tray-summary";

interface TrayPanelProps {
  reportingTimezone: string;
}

export function TrayPanel({ reportingTimezone }: TrayPanelProps) {
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
}: {
  summary: TraySummaryResponse;
  isRefreshing: boolean;
  isError: boolean;
  error: Error | null;
}) {
  const isEmpty = summary.dataStatus === "empty";

  return (
    <main className="min-h-screen overflow-hidden rounded-2xl border border-border bg-background text-foreground">
      <div className="flex flex-col gap-6 p-5">
        <header
          data-tauri-drag-region
          className="flex items-start justify-between gap-3"
        >
          <div>
            <p className="text-xs font-semibold tracking-wide text-muted-foreground uppercase">
              Burnly
            </p>
            <p className="mt-0.5 text-xs text-muted-foreground">
              Updated {formatDateTime(summary.lastSuccessfulRefreshAt)}
            </p>
          </div>
          <div className="flex items-center gap-1.5">
            <FreshnessStatus
              state={freshnessState(summary.dataStatus, isRefreshing, isError)}
            />
            <PanelCloseButton />
          </div>
        </header>

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

        <OpenDetailsButton
          onClick={() => {
            void openDetails();
          }}
        />
      </div>
    </main>
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

function freshnessState(
  dataStatus: TraySummaryResponse["dataStatus"],
  isRefreshing: boolean,
  isError: boolean,
): FreshnessState {
  if (isError) return "failed";
  if (isRefreshing) return "refreshing";
  return dataStatus;
}

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

function userSafeErrorMessage(error: unknown): string {
  return error instanceof Error
    ? error.message
    : "Burnly could not load tray summary data.";
}
