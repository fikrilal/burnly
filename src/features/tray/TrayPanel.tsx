import { openDetails } from "../../ipc/client";
import type { TraySummaryResponse } from "../../ipc/generated/contracts";
import {
  AllocationList,
  CompactCard,
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
import { formatDateTime, formatNumber } from "../../lib/format";
import { useTraySummary } from "./use-tray-summary";

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
    <main className="min-h-screen bg-background px-4 py-4 text-foreground">
      <CompactCard className="p-5">
        <header className="flex items-start justify-between gap-4">
          <div>
            <p className="text-xs font-semibold tracking-wide text-foreground uppercase">
              Burnly
            </p>
            <p className="mt-1 text-xs text-muted-foreground">
              Updated {formatDateTime(summary.lastSuccessfulRefreshAt)}
            </p>
          </div>
          <FreshnessStatus
            state={freshnessState(summary.dataStatus, isRefreshing, isError)}
          />
        </header>

        {isError ? (
          <ErrorState
            className="mt-4"
            title="Update failed"
            description={userSafeErrorMessage(error)}
          />
        ) : null}

        <div className="mt-7 space-y-5">
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
                value: formatNumber(summary.week.totalTokens),
              },
              {
                label: "This month",
                value: formatNumber(summary.month.totalTokens),
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

        <OpenDetailsButton
          className="mt-6"
          onClick={() => {
            void openDetails();
          }}
        />
      </CompactCard>
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
    <main className="min-h-screen bg-background px-4 py-4 text-foreground">
      <CompactCard className="p-5">
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
        <h1 className="mt-5 text-2xl font-semibold">Burnly</h1>
        <p className="mt-2 text-sm text-muted-foreground">{detail}</p>
      </CompactCard>
    </main>
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
