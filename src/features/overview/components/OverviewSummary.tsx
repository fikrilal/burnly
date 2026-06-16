import { formatNumber, formatCurrency } from "../../../lib/format";
import type { UsageOverviewResponse } from "../../../ipc/generated/contracts";

interface OverviewSummaryProps {
  overview: UsageOverviewResponse;
}

export function OverviewSummary({ overview }: OverviewSummaryProps) {
  return (
    <div className="grid gap-4 md:grid-cols-3">
      <div className="rounded-lg border border-zinc-800 bg-zinc-900/70 p-5">
        <p className="text-sm font-medium text-zinc-400">Total Tokens</p>
        <p className="mt-2 text-3xl font-semibold text-white">
          {formatNumber(overview.totalTokens)}
        </p>
      </div>
      <div className="rounded-lg border border-zinc-800 bg-zinc-900/70 p-5">
        <p className="text-sm font-medium text-zinc-400">Estimated Cost</p>
        <p className="mt-2 text-3xl font-semibold text-white">
          {formatCurrency(overview.cost.amountMicros, overview.cost.currency)}
        </p>
        {overview.cost.valuation !== "available" && (
          <p className="mt-1 text-xs text-zinc-500">
            Cost data {overview.cost.valuation}
          </p>
        )}
      </div>
      <div className="rounded-lg border border-zinc-800 bg-zinc-900/70 p-5">
        <p className="text-sm font-medium text-zinc-400">Active Days</p>
        <p className="mt-2 text-3xl font-semibold text-white">
          {overview.activeDays}
        </p>
      </div>
    </div>
  );
}
