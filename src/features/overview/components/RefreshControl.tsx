import { RefreshCw } from "lucide-react";
import { formatDateTime } from "../../../lib/format";
import type { UsageOverviewResponse } from "../../../ipc/generated/contracts";

interface RefreshControlProps {
  dataStatus: UsageOverviewResponse["dataStatus"];
  lastRefreshAt: string | null;
  onRefresh: () => void;
  isRefreshing: boolean;
}

export function RefreshControl({
  dataStatus,
  lastRefreshAt,
  onRefresh,
  isRefreshing,
}: RefreshControlProps) {
  const statusColors = {
    current: "text-green-400 bg-green-400/10",
    stale: "text-amber-400 bg-amber-400/10",
    partial: "text-yellow-400 bg-yellow-400/10",
    empty: "text-zinc-400 bg-zinc-400/10",
  };

  return (
    <div className="flex items-center justify-between rounded-lg border border-zinc-800 bg-zinc-900/70 p-4">
      <div>
        <div className="flex items-center gap-2">
          <p className="text-sm font-medium text-zinc-300">Status:</p>
          <span
            className={`inline-flex items-center rounded px-2 py-0.5 text-xs font-medium capitalize ${statusColors[dataStatus]}`}
          >
            {dataStatus}
          </span>
        </div>
        <p className="mt-1 text-xs text-zinc-500">
          Last updated: {formatDateTime(lastRefreshAt)}
        </p>
      </div>
      <button
        type="button"
        onClick={onRefresh}
        disabled={isRefreshing}
        className="inline-flex items-center justify-center gap-2 rounded-md bg-zinc-800 px-4 py-2 text-sm font-medium text-zinc-200 transition-colors hover:bg-zinc-700 hover:text-white disabled:pointer-events-none disabled:opacity-50"
      >
        <RefreshCw
          className={`h-4 w-4 ${isRefreshing ? "animate-spin" : ""}`}
        />
        {isRefreshing ? "Refreshing..." : "Refresh Now"}
      </button>
    </div>
  );
}
