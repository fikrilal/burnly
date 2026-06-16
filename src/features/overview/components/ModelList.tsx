import { formatNumber, formatCurrency } from "../../../lib/format";
import type { UsageOverviewResponse } from "../../../ipc/generated/contracts";

interface ModelListProps {
  models: UsageOverviewResponse["models"];
}

export function ModelList({ models }: ModelListProps) {
  if (models.length === 0) {
    return (
      <div className="rounded-lg border border-zinc-800 bg-zinc-900/40 p-8 text-center">
        <p className="text-zinc-500">No models recorded for this period.</p>
      </div>
    );
  }

  return (
    <div className="overflow-hidden rounded-lg border border-zinc-800 bg-zinc-900/70">
      <table className="w-full text-left text-sm text-zinc-400">
        <thead className="border-b border-zinc-800 bg-zinc-900 text-xs uppercase text-zinc-500">
          <tr>
            <th scope="col" className="px-6 py-4 font-medium text-white">
              Model
            </th>
            <th scope="col" className="px-6 py-4 font-medium text-white">
              Tokens
            </th>
            <th scope="col" className="px-6 py-4 font-medium text-white">
              Cost
            </th>
          </tr>
        </thead>
        <tbody className="divide-y divide-zinc-800">
          {models.map((model) => (
            <tr key={model.name} className="hover:bg-zinc-800/50">
              <td className="px-6 py-4 font-medium text-zinc-200">
                {model.name}
              </td>
              <td className="px-6 py-4">{formatNumber(model.totalTokens)}</td>
              <td className="px-6 py-4">
                {formatCurrency(model.cost.amountMicros, model.cost.currency)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
