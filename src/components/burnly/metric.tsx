import type { ReactNode } from "react";

import { cn } from "@/lib/cn";

interface CompactMetricProps {
  label: string;
  value: ReactNode;
  caption?: ReactNode;
  className?: string;
}

export function CompactMetric({
  label,
  value,
  caption,
  className,
}: CompactMetricProps) {
  return (
    <div className={className}>
      <p className="text-sm font-medium text-muted-foreground">{label}</p>
      <p className="mt-1.5 text-4xl font-semibold tracking-tight text-foreground tabular-nums">
        {value}
      </p>
      {caption ? (
        <p className="mt-1 text-xs text-muted-foreground">{caption}</p>
      ) : null}
    </div>
  );
}

export interface MetricRowItem {
  label: string;
  value: ReactNode;
}

interface MetricRowProps {
  items: MetricRowItem[];
  className?: string;
}

export function MetricRow({ items, className }: MetricRowProps) {
  return (
    <div className={cn("grid grid-cols-2 gap-4", className)}>
      {items.map((item) => (
        <div key={item.label}>
          <p className="text-xs text-muted-foreground">{item.label}</p>
          <p className="mt-1 text-lg font-semibold text-foreground tabular-nums">
            {item.value}
          </p>
        </div>
      ))}
    </div>
  );
}
