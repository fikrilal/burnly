import { useState } from "react";
import { useOverview } from "./use-overview";
import { OverviewSummary } from "./components/OverviewSummary";
import { SourceList } from "./components/SourceList";
import { ModelList } from "./components/ModelList";
import { RefreshHeader } from "./components/RefreshHeader";
import { EmptyState } from "./components/EmptyState";
import { AlertCircle } from "lucide-react";

function useDateRange() {
  const [dateRange] = useState(() => {
    const end = new Date();
    const start = new Date(Date.now() - 30 * 24 * 60 * 60 * 1000);
    return {
      startDate: start.toISOString().substring(0, 10),
      endDate: end.toISOString().substring(0, 10),
    };
  });
  return dateRange;
}

export function Overview() {
  const [isRefreshing, setIsRefreshing] = useState(false);
  const dateRange = useDateRange();

  const [reportingTimezone] = useState(
    () => Intl.DateTimeFormat().resolvedOptions().timeZone,
  );

  const {
    data,
    manualRefresh,
    isPending,
    isError,
    error,
    refetch,
    refreshError,
  } = useOverview({
    startDate: dateRange.startDate,
    endDate: dateRange.endDate,
    reportingTimezone,
  });

  const handleRefresh = async () => {
    setIsRefreshing(true);
    try {
      await manualRefresh();
    } finally {
      setIsRefreshing(false);
    }
  };

  if (isPending) {
    return (
      <div className="flex h-64 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900/50">
        <p className="text-zinc-500">Loading overview...</p>
      </div>
    );
  }

  if (isError) {
    return (
      <div className="flex flex-col items-center justify-center rounded-lg border border-red-900/50 bg-red-950/20 p-8 text-center">
        <AlertCircle className="mb-4 h-8 w-8 text-red-500" aria-hidden />
        <p className="font-medium text-red-400">Failed to load overview data</p>
        <p className="mt-2 text-sm text-red-500/70 max-w-md">{String(error)}</p>
        <button
          type="button"
          onClick={() => void refetch()}
          className="mt-6 inline-flex items-center justify-center rounded-md bg-red-900/50 px-4 py-2 text-sm font-medium text-red-200 transition-colors hover:bg-red-900/70"
        >
          Retry
        </button>
      </div>
    );
  }

  const isEmpty = data.dataStatus === "empty";

  return (
    <div className="flex flex-col gap-8">
      <div className="flex items-start justify-between">
        <div>
          <h2 className="text-2xl font-semibold tracking-tight text-white">
            Overview
          </h2>
          {!isEmpty && (
            <p className="mt-1 text-sm text-zinc-400">
              Usage data from {data.period.startDate} to {data.period.endDate}
            </p>
          )}
        </div>

        {!isEmpty && (
          <RefreshHeader
            dataStatus={data.dataStatus}
            lastRefreshAt={data.lastSuccessfulRefreshAt}
            onRefresh={() => void handleRefresh()}
            isRefreshing={isRefreshing}
          />
        )}
      </div>

      {refreshError && (
        <div className="flex items-start gap-3 rounded-lg border border-red-900/50 bg-red-950/20 p-4">
          <AlertCircle
            className="h-5 w-5 text-red-500 mt-0.5 shrink-0"
            aria-hidden
          />
          <div>
            <p className="text-sm font-medium text-red-400">Refresh Failed</p>
            <p className="mt-1 text-sm text-red-500/70">
              {String(refreshError)}. Displaying last successful data.
            </p>
          </div>
        </div>
      )}

      {isEmpty ? (
        <EmptyState
          onRefresh={() => void handleRefresh()}
          isRefreshing={isRefreshing}
        />
      ) : (
        <>
          <OverviewSummary overview={data} />

          <div className="flex flex-col gap-4">
            <h3 className="text-lg font-medium text-white">Sources</h3>
            <SourceList sources={data.sources} />
          </div>

          <div className="flex flex-col gap-4">
            <h3 className="text-lg font-medium text-white">Models</h3>
            <ModelList models={data.models} />
          </div>
        </>
      )}
    </div>
  );
}
