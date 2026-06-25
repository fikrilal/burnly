import { AlertCircle, ExternalLink } from "lucide-react";

import { openDetails } from "../../ipc/client";
import type { TraySummaryResponse } from "../../ipc/generated/contracts";
import { CompactCard, StatusPill } from "../../components/burnly";
import { formatDateTime } from "../../lib/format";
import { useTraySummary } from "./use-tray-summary";
import { ModelUsageAllocation } from "./components/ModelUsageAllocation";
import { PrimaryMetric, SecondaryMetricRow } from "./components/TrayMetric";

interface TrayPanelProps {
  reportingTimezone: string;
}

export function TrayPanel({ reportingTimezone }: TrayPanelProps) {
  const summary = useTraySummary(reportingTimezone);

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
    <main className="min-h-screen bg-zinc-950 px-4 py-4 text-zinc-50">
      <CompactCard className="p-5">
        <FreshnessHeader
          summary={summary}
          isRefreshing={isRefreshing}
          isError={isError}
        />

        {isError ? <InlineError message={userSafeErrorMessage(error)} /> : null}

        <div className="mt-7 space-y-5">
          <PrimaryMetric totalTokens={summary.today.totalTokens} />
          <SecondaryMetricRow
            weekTokens={summary.week.totalTokens}
            monthTokens={summary.month.totalTokens}
          />
          {isEmpty ? <EmptyUsage /> : null}
          <ModelUsageAllocation models={summary.models} />
        </div>

        <button
          type="button"
          onClick={() => {
            void openDetails();
          }}
          className="mt-6 inline-flex w-full items-center justify-center gap-2 rounded-xl bg-cyan-500 px-4 py-3 text-sm font-semibold text-zinc-950 transition-colors hover:bg-cyan-400 focus:outline-none focus:ring-2 focus:ring-cyan-300 focus:ring-offset-2 focus:ring-offset-zinc-950"
        >
          Open details
          <ExternalLink className="h-4 w-4" aria-hidden />
        </button>
      </CompactCard>
    </main>
  );
}

function FreshnessHeader({
  summary,
  isRefreshing,
  isError,
}: {
  summary: TraySummaryResponse;
  isRefreshing: boolean;
  isError: boolean;
}) {
  return (
    <header className="flex items-start justify-between gap-4">
      <div>
        <p className="text-xs font-semibold uppercase tracking-wide text-cyan-300">
          Burnly
        </p>
        <p className="mt-1 text-xs text-zinc-500">
          Updated {formatDateTime(summary.lastSuccessfulRefreshAt)}
        </p>
      </div>
      <StatusPill tone={statusTone(summary.dataStatus, isRefreshing, isError)}>
        {statusLabel(summary.dataStatus, isRefreshing, isError)}
      </StatusPill>
    </header>
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
    <main className="min-h-screen bg-zinc-950 px-4 py-4 text-zinc-50">
      <CompactCard className="p-5">
        <StatusPill tone={tone}>{status}</StatusPill>
        <h1 className="mt-5 text-2xl font-semibold">Burnly</h1>
        <p className="mt-2 text-sm text-zinc-400">{detail}</p>
      </CompactCard>
    </main>
  );
}

function EmptyUsage() {
  return (
    <div className="rounded-xl border border-zinc-800 bg-zinc-950/40 p-4">
      <p className="text-sm font-medium text-zinc-200">No data collected</p>
      <p className="mt-1 text-xs text-zinc-500">
        Burnly refreshes automatically when usage data becomes stale.
      </p>
    </div>
  );
}

function InlineError({ message }: { message: string }) {
  return (
    <div className="mt-4 flex gap-2 rounded-xl border border-red-900/50 bg-red-950/20 p-3 text-sm text-red-200">
      <AlertCircle className="mt-0.5 h-4 w-4 shrink-0" aria-hidden />
      <p>{message}</p>
    </div>
  );
}

function statusLabel(
  dataStatus: TraySummaryResponse["dataStatus"],
  isRefreshing: boolean,
  isError: boolean,
): string {
  if (isError) return "Update failed";
  if (isRefreshing) return "Refreshing";
  switch (dataStatus) {
    case "current":
      return "Current";
    case "stale":
      return "Stale";
    case "partial":
      return "Partial";
    case "empty":
      return "Empty";
  }
}

function statusTone(
  dataStatus: TraySummaryResponse["dataStatus"],
  isRefreshing: boolean,
  isError: boolean,
) {
  if (isError) return "danger";
  if (isRefreshing) return "warning";
  switch (dataStatus) {
    case "current":
      return "success";
    case "stale":
    case "partial":
      return "warning";
    case "empty":
      return "neutral";
  }
}

function userSafeErrorMessage(error: unknown): string {
  return error instanceof Error
    ? error.message
    : "Burnly could not load tray summary data.";
}
