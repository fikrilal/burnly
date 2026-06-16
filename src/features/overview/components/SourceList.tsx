import { formatNumber, formatCurrency } from "../../../lib/format";
import type { UsageOverviewResponse } from "../../../ipc/generated/contracts";

interface SourceListProps {
  sources: UsageOverviewResponse["sources"];
}

export function SourceList({ sources }: SourceListProps) {
  if (sources.length === 0) {
    return (
      <div className="rounded-lg border border-zinc-800 bg-zinc-900/40 p-8 text-center">
        <p className="text-zinc-500">No sources recorded for this period.</p>
      </div>
    );
  }

  return (
    <div className="overflow-hidden rounded-lg border border-zinc-800 bg-zinc-900/70">
      <table className="w-full text-left text-sm text-zinc-400">
        <thead className="border-b border-zinc-800 bg-zinc-900 text-xs uppercase text-zinc-500">
          <tr>
            <th scope="col" className="px-6 py-4 font-medium text-white">
              Source
            </th>
            <th scope="col" className="px-6 py-4 font-medium text-white">
              Tokens
            </th>
            <th scope="col" className="px-6 py-4 font-medium text-white">
              Cost
            </th>
            <th scope="col" className="px-6 py-4 font-medium text-white">
              Active Days
            </th>
          </tr>
        </thead>
        <tbody className="divide-y divide-zinc-800">
          {sources.map((source) => (
            <tr key={source.source} className="hover:bg-zinc-800/50">
              <td className="px-6 py-4 font-medium text-zinc-200">
                {source.source}
                {source.hasPartialData && (
                  <span className="ml-2 inline-flex items-center rounded bg-amber-400/10 px-2 py-0.5 text-xs font-medium text-amber-400">
                    Partial
                  </span>
                )}
              </td>
              <td className="px-6 py-4">{formatNumber(source.totalTokens)}</td>
              <td className="px-6 py-4">
                {formatCurrency(source.cost.amountMicros, source.cost.currency)}
              </td>
              <td className="px-6 py-4">{source.activeDays}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
