import { formatNumber } from "../../../lib/format";
import type { TraySummaryResponse } from "../../../ipc/generated/contracts";

const accents = [
  "bg-blue-500",
  "bg-emerald-500",
  "bg-violet-500",
  "bg-zinc-500",
];

interface ModelUsageAllocationProps {
  models: TraySummaryResponse["models"];
}

export function ModelUsageAllocation({ models }: ModelUsageAllocationProps) {
  if (models.length === 0) {
    return (
      <div className="rounded-xl border border-dashed border-zinc-800 bg-zinc-950/30 p-5 text-center">
        <p className="text-sm font-medium text-zinc-300">
          No model usage today
        </p>
        <p className="mt-1 text-xs text-zinc-500">
          Burnly will populate this after usage is collected.
        </p>
      </div>
    );
  }

  return (
    <div>
      <p className="text-xs font-semibold uppercase tracking-wide text-zinc-500">
        Model usage today
      </p>
      <div className="mt-3 space-y-4">
        {models.map((model, index) => (
          <ModelRow
            key={`${model.modelName}-${model.agentLabel}`}
            model={model}
            accentClass={accentClass(index)}
          />
        ))}
      </div>
    </div>
  );
}

function accentClass(index: number): string {
  return accents[index] ?? "bg-zinc-500";
}

function ModelRow({
  model,
  accentClass,
}: {
  model: TraySummaryResponse["models"][number];
  accentClass: string;
}) {
  return (
    <div className="grid grid-cols-[4px_1fr_auto] gap-3">
      <div className={`rounded-full ${accentClass}`} aria-hidden />
      <div className="min-w-0">
        <p className="truncate text-sm font-semibold text-zinc-100">
          {model.modelName}
        </p>
        <p className="mt-1 truncate text-xs text-zinc-500">
          {model.agentLabel}
        </p>
      </div>
      <div className="text-right">
        <p className="text-sm font-semibold text-zinc-100">
          {formatNumber(model.totalTokens)}
        </p>
        <TrendLabel trend={model.trend} />
      </div>
    </div>
  );
}

function TrendLabel({
  trend,
}: {
  trend: TraySummaryResponse["models"][number]["trend"];
}) {
  if (!trend) {
    return <p className="mt-1 text-xs text-zinc-500">new today</p>;
  }

  const value = `${formatTrendBasisPoints(trend.basisPoints)}%`;
  if (trend.direction === "flat") {
    return <p className="mt-1 text-xs text-zinc-500">→ {value}</p>;
  }

  return (
    <p
      className={`mt-1 text-xs ${
        trend.direction === "increased" ? "text-emerald-400" : "text-red-400"
      }`}
    >
      {trend.direction === "increased" ? "↗" : "↘"} {value}
    </p>
  );
}

function formatTrendBasisPoints(basisPoints: number): string {
  const percentage = basisPoints / 100;
  return Number.isInteger(percentage)
    ? percentage.toFixed(0)
    : percentage.toFixed(1);
}
