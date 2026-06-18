import { RefreshCw } from "lucide-react";
import { formatDateTime } from "../../../lib/format";
import type {
  RefreshStatusResponse,
  UsageOverviewResponse,
} from "../../../ipc/generated/contracts";

interface RefreshHeaderProps {
  dataStatus: UsageOverviewResponse["dataStatus"];
  lastRefreshAt: string | null;
  onRefresh: () => void;
  isRefreshing: boolean;
  refreshStatus: RefreshStatusResponse["status"] | null;
}

export function RefreshHeader({
  dataStatus,
  lastRefreshAt,
  onRefresh,
  isRefreshing,
  refreshStatus,
}: RefreshHeaderProps) {
  const statusColors = {
    current: "text-green-400 bg-green-400/10 border-green-400/20",
    stale: "text-amber-400 bg-amber-400/10 border-amber-400/20",
    partial: "text-yellow-400 bg-yellow-400/10 border-yellow-400/20",
    empty: "text-zinc-400 bg-zinc-400/10 border-zinc-400/20",
  };

  return (
    <div className="flex items-center gap-4 text-right">
      <div className="flex flex-col items-end">
        <span
          className={`inline-flex items-center rounded border px-2 py-0.5 text-[11px] font-medium uppercase tracking-wider ${statusColors[dataStatus]}`}
        >
          {dataStatus}
        </span>
        <p className="mt-1 text-xs text-zinc-500">
          {refreshStatus && isRefreshing
            ? `Refresh ${refreshStatus}`
            : `Last updated: ${formatDateTime(lastRefreshAt)}`}
        </p>
      </div>
      <button
        type="button"
        onClick={onRefresh}
        disabled={isRefreshing}
        className="inline-flex h-9 w-9 items-center justify-center rounded-md border border-zinc-800 bg-zinc-900/50 text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-white disabled:pointer-events-none disabled:opacity-50"
        title="Refresh Now"
        aria-label="Refresh Now"
      >
        <RefreshCw
          className={`h-4 w-4 ${isRefreshing ? "animate-spin" : ""}`}
        />
      </button>
    </div>
  );
}
