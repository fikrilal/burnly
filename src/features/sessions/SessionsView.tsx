import { useEffect, useRef, useState } from "react";
import { AlertCircle } from "lucide-react";
import { useSessions } from "./use-sessions";
import { SessionDetailCard } from "./SessionDetailCard";
import { formatNumber, formatCurrency, formatDateTime } from "../../lib/format";

export function SessionsView() {
  const {
    data,
    fetchNextPage,
    hasNextPage,
    isFetchingNextPage,
    isPending,
    isError,
    error,
    refetch,
  } = useSessions({ sourceId: null, limit: 20 });

  const loadMoreRef = useRef<HTMLDivElement>(null);
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(
    null,
  );

  useEffect(() => {
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting && hasNextPage && !isFetchingNextPage) {
          void fetchNextPage();
        }
      },
      { threshold: 0.1 },
    );

    if (loadMoreRef.current) {
      observer.observe(loadMoreRef.current);
    }

    return () => {
      observer.disconnect();
    };
  }, [hasNextPage, isFetchingNextPage, fetchNextPage]);

  if (isPending) {
    return (
      <div className="flex h-64 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900/50">
        <p className="text-zinc-500">Loading sessions...</p>
      </div>
    );
  }

  if (isError) {
    return (
      <div className="flex flex-col items-center justify-center rounded-lg border border-red-900/50 bg-red-950/20 p-8 text-center">
        <AlertCircle className="mb-4 h-8 w-8 text-red-500" aria-hidden />
        <p className="font-medium text-red-400">Failed to load sessions</p>
        <p className="mt-2 text-sm text-red-500/70 max-w-md">{String(error)}</p>
        <button
          type="button"
          onClick={() => {
            void refetch();
          }}
          className="mt-6 inline-flex items-center justify-center rounded-md bg-red-900/50 px-4 py-2 text-sm font-medium text-red-200 transition-colors hover:bg-red-900/70"
        >
          Retry
        </button>
      </div>
    );
  }

  const items = data.pages.flatMap((page) => page.items);

  if (items.length === 0) {
    return (
      <div className="flex h-64 flex-col items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900/50 text-center">
        <p className="text-zinc-400">No sessions recorded yet.</p>
        <p className="mt-1 text-sm text-zinc-500">
          Sessions will appear here once your collectors sync data.
        </p>
      </div>
    );
  }

  return (
    <div className="flex flex-col md:flex-row gap-6 items-start">
      <div className="w-full md:w-1/2 flex flex-col gap-4">
        <h2 className="text-xl font-semibold text-white">Sessions</h2>
        <div className="flex flex-col gap-3">
          {items.map((session) => (
            <button
              key={session.id}
              onClick={() => {
                setSelectedSessionId(session.id);
              }}
              className={`text-left block rounded-lg border p-4 transition-colors ${
                selectedSessionId === session.id
                  ? "border-cyan-500/50 bg-cyan-950/20"
                  : "border-zinc-800 bg-zinc-900/50 hover:border-zinc-700"
              }`}
            >
              <div className="flex justify-between items-start mb-2">
                <span className="font-medium text-white truncate pr-4">
                  {session.projectPath ?? session.label}
                </span>
                <span className="text-sm font-semibold text-zinc-300 shrink-0">
                  {formatNumber(session.totalTokens)}
                </span>
              </div>
              <div className="flex justify-between items-center text-xs text-zinc-500">
                <span>
                  {session.lastActivityAt
                    ? formatDateTime(session.lastActivityAt)
                    : "Unknown time"}
                </span>
                {session.cost.valuation !== "unavailable" && (
                  <span>
                    {formatCurrency(
                      session.cost.amountMicros ?? null,
                      session.cost.currency ?? null,
                    )}
                  </span>
                )}
              </div>
            </button>
          ))}
          <div
            ref={loadMoreRef}
            className="h-10 flex items-center justify-center"
          >
            {isFetchingNextPage ? (
              <span className="text-sm text-zinc-500">Loading more...</span>
            ) : hasNextPage ? (
              <span className="text-sm text-zinc-600">Scroll for more</span>
            ) : (
              <span className="text-sm text-zinc-600">No more sessions</span>
            )}
          </div>
        </div>
      </div>

      <div className="w-full md:w-1/2 sticky top-6">
        {selectedSessionId ? (
          <SessionDetailCard sessionId={selectedSessionId} />
        ) : (
          <div className="rounded-lg border border-zinc-800 border-dashed bg-zinc-900/20 p-8 text-center text-zinc-500">
            Select a session to view details
          </div>
        )}
      </div>
    </div>
  );
}
