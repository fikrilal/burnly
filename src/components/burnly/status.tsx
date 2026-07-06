import type { ReactNode } from "react";
import {
  AlertCircle,
  AlertTriangle,
  Check,
  Clock,
  ExternalLink,
  Inbox,
  RefreshCw,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/cn";

export type FreshnessState =
  | "current"
  | "stale"
  | "partial"
  | "refreshing"
  | "failed"
  | "empty";

const FRESHNESS: Record<FreshnessState, { label: string; Icon: LucideIcon }> = {
  current: { label: "Current", Icon: Check },
  stale: { label: "Stale", Icon: Clock },
  partial: { label: "Some sources failed", Icon: AlertTriangle },
  refreshing: { label: "Refreshing", Icon: RefreshCw },
  failed: { label: "Refresh failed", Icon: AlertCircle },
  empty: { label: "Empty", Icon: Inbox },
};

export function FreshnessStatus({
  state,
  className,
}: {
  state: FreshnessState;
  className?: string;
}) {
  const { label, Icon } = FRESHNESS[state];
  const danger = state === "failed";

  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs font-medium",
        danger
          ? "bg-destructive/10 text-destructive"
          : "bg-muted text-muted-foreground",
        className,
      )}
    >
      <Icon
        className={cn(
          "size-3",
          state === "refreshing" && "animate-spin motion-reduce:animate-none",
        )}
        aria-hidden
      />
      {label}
    </span>
  );
}

export function EmptyState({
  icon: Icon = Inbox,
  title,
  description,
  className,
}: {
  icon?: LucideIcon;
  title: string;
  description?: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "rounded-xl border border-dashed border-border bg-card/40 p-6 text-center",
        className,
      )}
    >
      <Icon className="mx-auto size-5 text-muted-foreground" aria-hidden />
      <p className="mt-2 text-sm font-medium text-foreground">{title}</p>
      {description ? (
        <p className="mt-1 text-xs text-muted-foreground">{description}</p>
      ) : null}
    </div>
  );
}

export function ErrorState({
  title,
  description,
  action,
  className,
}: {
  title: string;
  description?: ReactNode;
  action?: ReactNode;
  className?: string;
}) {
  return (
    <div
      role="alert"
      className={cn(
        "flex gap-2 rounded-xl border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive",
        className,
      )}
    >
      <AlertCircle className="mt-0.5 size-4 shrink-0" aria-hidden />
      <div className="min-w-0">
        <p className="font-medium">{title}</p>
        {description ? (
          <p className="mt-0.5 text-destructive/80">{description}</p>
        ) : null}
        {action ? <div className="mt-2">{action}</div> : null}
      </div>
    </div>
  );
}

export function OpenDetailsButton({
  onClick,
  label = "Open details",
  className,
}: {
  onClick?: () => void;
  label?: string;
  className?: string;
}) {
  return (
    <Button
      variant="ghost"
      size="xs"
      onClick={onClick}
      className={cn("text-muted-foreground hover:text-foreground", className)}
    >
      {label}
      <ExternalLink aria-hidden />
    </Button>
  );
}
