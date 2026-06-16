import { formatNumber, formatCurrency, formatDateTime } from "../../lib/format";
import { useSessionDetail } from "./use-sessions";

interface SessionDetailCardProps {
  sessionId: number;
}

export function SessionDetailCard({ sessionId }: SessionDetailCardProps) {
  const { data, isPending, isError } = useSessionDetail(sessionId);

  if (isPending) {
    return (
      <div className="rounded-lg border border-zinc-800 bg-zinc-900/50 p-4 animate-pulse">
        <div className="h-4 bg-zinc-800 rounded w-1/4 mb-4"></div>
        <div className="h-4 bg-zinc-800 rounded w-1/2 mb-2"></div>
        <div className="h-4 bg-zinc-800 rounded w-1/3"></div>
      </div>
    );
  }

  if (isError || !data) {
    return (
      <div className="rounded-lg border border-red-900/50 bg-red-950/20 p-4 text-sm text-red-400">
        Failed to load session details
      </div>
    );
  }

  const { session, models } = data;

  return (
    <div className="rounded-lg border border-zinc-800 bg-zinc-900/50 p-6">
      <div className="flex items-start justify-between border-b border-zinc-800 pb-4 mb-4">
        <div>
          <h3 className="text-lg font-medium text-white mb-1">
            {session.projectPath ?? session.sourceSessionId}
          </h3>
          <p className="text-sm text-zinc-500">
            {session.firstActivityAt
              ? formatDateTime(session.firstActivityAt)
              : "Unknown"}
            {" - "}
            {session.lastActivityAt
              ? formatDateTime(session.lastActivityAt)
              : "Unknown"}
          </p>
        </div>
        <div className="text-right">
          <p className="text-lg font-semibold text-white">
            {formatNumber(session.totalTokens)} tokens
          </p>
          {session.cost.valuation !== "unavailable" && (
            <p className="text-sm text-zinc-400">
              {formatCurrency(
                session.cost.amountMicros ?? null,
                session.cost.currency ?? null,
              )}
            </p>
          )}
        </div>
      </div>

      <div>
        <h4 className="text-sm font-medium text-zinc-400 mb-3 uppercase tracking-wider">
          Models Used
        </h4>
        {models.length === 0 ? (
          <p className="text-sm text-zinc-600">
            No specific model data available.
          </p>
        ) : (
          <ul className="space-y-3">
            {models.map((m, idx) => (
              <li
                key={idx}
                className="flex justify-between items-center text-sm"
              >
                <span className="text-zinc-300 font-medium">
                  {m.rawModelId ?? "Unknown"}
                </span>
                <span className="text-zinc-500">
                  {formatNumber(m.totalTokens)}
                  {m.cost.valuation !== "unavailable" &&
                    ` • ${formatCurrency(m.cost.amountMicros ?? null, m.cost.currency ?? null)}`}
                </span>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
