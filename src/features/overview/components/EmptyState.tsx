import { Database } from "lucide-react";

interface EmptyStateProps {
  onRefresh: () => void;
  isRefreshing: boolean;
}

export function EmptyState({ onRefresh, isRefreshing }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900/50 py-16 text-center">
      <div className="flex h-12 w-12 items-center justify-center rounded-full bg-zinc-800">
        <Database className="h-6 w-6 text-zinc-400" aria-hidden />
      </div>
      <h3 className="mt-4 text-lg font-medium text-white">No data collected</h3>
      <p className="mt-2 text-sm text-zinc-400 max-w-sm">
        There are no token usage records for this period. Click refresh to query
        active collectors for new data.
      </p>
      <button
        type="button"
        onClick={onRefresh}
        disabled={isRefreshing}
        className="mt-6 inline-flex items-center justify-center rounded-md bg-cyan-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-cyan-500 disabled:pointer-events-none disabled:opacity-50"
      >
        {isRefreshing ? "Refreshing..." : "Refresh Data"}
      </button>
    </div>
  );
}
