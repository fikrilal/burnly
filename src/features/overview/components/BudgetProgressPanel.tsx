import { AlertCircle } from "lucide-react";

import { formatCurrency, formatNumber } from "../../../lib/format";
import type {
  CurrentBudgetProgressItemResponse,
  CurrentBudgetProgressResponse,
} from "../../../ipc/generated/contracts";

interface BudgetProgressPanelProps {
  progress: CurrentBudgetProgressResponse | undefined;
  isPending: boolean;
  isError: boolean;
  isFetching: boolean;
  onRetry: () => void;
}

export function BudgetProgressPanel({
  progress,
  isPending,
  isError,
  isFetching,
  onRetry,
}: BudgetProgressPanelProps) {
  return (
    <section className="flex flex-col gap-4">
      <div className="flex items-center justify-between gap-4">
        <div>
          <h3 className="text-lg font-medium text-white">Budget progress</h3>
          {progress && (
            <p className="mt-1 text-sm text-zinc-500">
              Current budget periods in {progress.reportingTimezone}
            </p>
          )}
        </div>
        {isFetching && !isPending && (
          <span className="text-xs text-zinc-500">Updating...</span>
        )}
      </div>

      {isPending ? (
        <BudgetProgressShell>Loading budget progress...</BudgetProgressShell>
      ) : isError && !progress ? (
        <BudgetProgressError onRetry={onRetry} />
      ) : progress ? (
        <BudgetProgressContent progress={progress} />
      ) : null}

      {isError && progress && (
        <div className="flex items-start gap-3 rounded-lg border border-red-900/50 bg-red-950/20 p-4">
          <AlertCircle
            className="mt-0.5 h-5 w-5 shrink-0 text-red-500"
            aria-hidden
          />
          <div>
            <p className="text-sm font-medium text-red-400">
              Budget progress update failed
            </p>
            <p className="mt-1 text-sm text-red-500/70">
              Displaying the last successful budget progress.
            </p>
          </div>
        </div>
      )}
    </section>
  );
}

function BudgetProgressContent({
  progress,
}: {
  progress: CurrentBudgetProgressResponse;
}) {
  if (progress.status === "no_budgets") {
    return (
      <BudgetProgressShell>
        No budgets configured. Create one from the Budgets tab.
      </BudgetProgressShell>
    );
  }

  if (progress.status === "all_disabled") {
    return (
      <BudgetProgressShell>
        All configured budgets are disabled.
      </BudgetProgressShell>
    );
  }

  if (progress.items.length === 0) {
    return (
      <BudgetProgressShell>
        Budget progress is available, but there is no current usage to display.
      </BudgetProgressShell>
    );
  }

  return (
    <div className="grid gap-4 lg:grid-cols-2">
      {progress.items.map((item) => (
        <BudgetProgressCard key={item.budgetId} item={item} />
      ))}
    </div>
  );
}

function BudgetProgressCard({
  item,
}: {
  item: CurrentBudgetProgressItemResponse;
}) {
  return (
    <article className="rounded-lg border border-zinc-800 bg-zinc-900/70 p-5">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h4 className="font-medium text-white">{item.budgetName}</h4>
          <p className="mt-1 text-xs text-zinc-500">
            {item.period} · {item.periodStartDate} to {item.periodEndDate}
          </p>
        </div>
        <span
          className={
            item.exceeded
              ? "rounded-full border border-red-800 bg-red-950/40 px-2 py-1 text-xs font-medium text-red-300"
              : "rounded-full border border-zinc-700 bg-zinc-950/40 px-2 py-1 text-xs font-medium text-zinc-300"
          }
        >
          {item.exceeded ? "Exceeded" : progressPercentLabel(item)}
        </span>
      </div>

      <div className="mt-4">
        <div className="h-2 overflow-hidden rounded-full bg-zinc-800">
          <div
            className={
              item.exceeded ? "h-full bg-red-500" : "h-full bg-emerald-500"
            }
            style={{ width: `${progressBarWidth(item)}%` }}
          />
        </div>
        <div className="mt-3 flex items-center justify-between text-sm">
          <span className="text-zinc-400">{currentLabel(item)}</span>
          <span className="text-zinc-500">limit {limitLabel(item)}</span>
        </div>
        {item.state === "cost_unavailable" && (
          <p className="mt-3 text-sm text-amber-300">
            Cost is unavailable for this period
            {item.unavailableDays > 0
              ? ` (${item.unavailableDays} unavailable day${item.unavailableDays === 1 ? "" : "s"})`
              : ""}
            .
          </p>
        )}
        {item.completeness === "partial" && (
          <p className="mt-3 text-sm text-amber-300">
            Cost progress is partial because {item.unavailableDays} day
            {item.unavailableDays === 1 ? "" : "s"} could not be valued.
          </p>
        )}
      </div>
    </article>
  );
}

function BudgetProgressShell({ children }: { children: string }) {
  return (
    <div className="rounded-lg border border-zinc-800 bg-zinc-900/70 p-5 text-sm text-zinc-400">
      {children}
    </div>
  );
}

function BudgetProgressError({ onRetry }: { onRetry: () => void }) {
  return (
    <div className="flex items-start justify-between gap-4 rounded-lg border border-red-900/50 bg-red-950/20 p-4">
      <div className="flex items-start gap-3">
        <AlertCircle
          className="mt-0.5 h-5 w-5 shrink-0 text-red-500"
          aria-hidden
        />
        <div>
          <p className="text-sm font-medium text-red-400">
            Budget progress unavailable
          </p>
          <p className="mt-1 text-sm text-red-500/70">
            Burnly could not calculate current budget progress.
          </p>
        </div>
      </div>
      <button
        type="button"
        onClick={onRetry}
        className="shrink-0 rounded-md bg-red-900/50 px-3 py-2 text-sm font-medium text-red-200 transition-colors hover:bg-red-900/70"
      >
        Retry
      </button>
    </div>
  );
}

function progressPercentLabel(item: CurrentBudgetProgressItemResponse): string {
  if (!item.basisPoints) return "Unavailable";
  return `${Number(item.basisPoints) / 100}%`;
}

function progressBarWidth(item: CurrentBudgetProgressItemResponse): number {
  if (!item.basisPoints) return 0;
  return Math.min(Number(item.basisPoints) / 100, 100);
}

function currentLabel(item: CurrentBudgetProgressItemResponse): string {
  if (item.current === null) return "current unavailable";
  if (item.metric === "tokens") return `${formatNumber(item.current)} tokens`;
  return formatCurrency(item.current, item.currency);
}

function limitLabel(item: CurrentBudgetProgressItemResponse): string {
  if (item.metric === "tokens") return `${formatNumber(item.limit)} tokens`;
  return formatCurrency(item.limit, item.currency);
}
