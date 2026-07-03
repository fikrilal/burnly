import type { TraySummaryResponse } from "../../ipc/generated/contracts";
import {
  AllocationList,
  CompactMetric,
  EmptyState,
  ErrorState,
  MetricRow,
} from "../../components/burnly";
import { AnimatedNumber } from "../../components/ui/animated-number";
import { formatCompactNumber, formatNumber } from "../../lib/format";
import { userSafeErrorMessage } from "../../lib/user-safe-error";
import { toModelUsage, tokenNumber } from "./tray-utils";

export function OverviewTab({
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
