import type { ReactNode } from "react";
import { Minus, TrendingDown, TrendingUp } from "lucide-react";

import { cn } from "@/lib/cn";

export type TrendDirection = "increased" | "decreased" | "flat";

export interface Trend {
  direction: TrendDirection;
  basisPoints: number;
}

export interface ModelUsage {
  modelName: string;
  agentLabel: string;
  tokens: ReactNode;
  trend?: Trend | null;
}

// Monochrome rank accents: emphasis fades down the ranking.
const RANK_ACCENTS = [
  "bg-foreground",
  "bg-foreground/70",
  "bg-foreground/45",
  "bg-muted-foreground",
];

export function TrendIndicator({
  trend,
  className,
}: {
  trend?: Trend | null;
  className?: string;
}) {
  if (!trend) {
    return (
      <span className={cn("text-xs text-muted-foreground", className)}>
        new today
      </span>
    );
  }

  const Icon =
    trend.direction === "increased"
      ? TrendingUp
      : trend.direction === "decreased"
        ? TrendingDown
        : Minus;

  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 text-xs text-muted-foreground",
        className,
      )}
    >
      <Icon className="size-3" aria-hidden />
      {formatTrendPercent(trend.basisPoints)}%
    </span>
  );
}

export function AllocationList({
  models,
  title = "Model usage today",
  emptyLabel = "No model usage today",
  className,
}: {
  models: ModelUsage[];
  title?: string;
  emptyLabel?: string;
  className?: string;
}) {
  if (models.length === 0) {
    return (
      <div
        className={cn(
          "rounded-xl border border-dashed border-border bg-card/40 p-5 text-center",
          className,
        )}
      >
        <p className="text-sm font-medium text-foreground">{emptyLabel}</p>
        <p className="mt-1 text-xs text-muted-foreground">
          Burnly will populate this after usage is collected.
        </p>
      </div>
    );
  }

  return (
    <div className={className}>
      <p className="text-xs font-semibold tracking-wide text-muted-foreground uppercase">
        {title}
      </p>
      <div className="mt-3 space-y-4">
        {models.map((model, index) => (
          <ModelUsageRow
            key={`${model.modelName}-${model.agentLabel}`}
            model={model}
            accentClass={RANK_ACCENTS[index] ?? "bg-muted-foreground"}
          />
        ))}
      </div>
    </div>
  );
}

function ModelUsageRow({
  model,
  accentClass,
}: {
  model: ModelUsage;
  accentClass: string;
}) {
  return (
    <div className="grid grid-cols-[3px_1fr_auto] gap-3">
      <div className={cn("rounded-full", accentClass)} aria-hidden />
      <div className="min-w-0">
        <p className="truncate text-sm font-semibold text-foreground">
          {model.modelName}
        </p>
        <p className="mt-1 truncate text-xs text-muted-foreground">
          {model.agentLabel}
        </p>
      </div>
      <div className="text-right">
        <p className="text-sm font-semibold text-foreground">{model.tokens}</p>
        <TrendIndicator trend={model.trend ?? null} className="mt-1" />
      </div>
    </div>
  );
}

function formatTrendPercent(basisPoints: number): string {
  const percentage = basisPoints / 100;
  return Number.isInteger(percentage)
    ? percentage.toFixed(0)
    : percentage.toFixed(1);
}
