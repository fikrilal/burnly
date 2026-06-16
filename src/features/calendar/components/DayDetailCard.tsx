import { AlertCircle } from "lucide-react";
import type { DayDetailResponse } from "../../../ipc/generated/contracts";
import { formatNumber, formatCurrency } from "../../../lib/format";

interface DayDetailCardProps {
  detail: DayDetailResponse;
}

export function DayDetailCard({ detail }: DayDetailCardProps) {
  const isPartial = detail.cost.completeness === "partial";
  const isUnavailable = detail.cost.completeness === "unavailable";

  return (
    <div className="rounded-lg border border-zinc-800 bg-zinc-900/50 p-6">
      <div className="mb-6">
        <h3 className="text-lg font-medium text-white">{detail.date}</h3>
        <div className="mt-2 flex items-baseline gap-2">
          <span className="text-3xl font-semibold text-white">
            {formatNumber(detail.totalTokens)}
          </span>
          <span className="text-sm text-zinc-400">tokens</span>
        </div>

        {detail.cost.amountMicros && detail.cost.currency && (
          <div className="mt-1 text-sm text-zinc-500">
            Estimated cost:{" "}
            {formatCurrency(detail.cost.amountMicros, detail.cost.currency)}
          </div>
        )}

        {(isPartial || isUnavailable) && (
          <div className="mt-3 flex items-start gap-2 rounded border border-amber-900/30 bg-amber-900/10 p-3 text-amber-500/90 text-sm">
            <AlertCircle className="h-4 w-4 shrink-0 mt-0.5" />
            <p>
              {isUnavailable
                ? "Cost data is currently unavailable for this date."
                : "Cost data is incomplete for some models on this date."}
            </p>
          </div>
        )}
      </div>

      <div className="space-y-4">
        <h4 className="text-sm font-medium text-zinc-400 uppercase tracking-wider">
          Models Used
        </h4>

        {detail.models.length === 0 ? (
          <p className="text-sm text-zinc-500">
            No specific model usage recorded for this date.
          </p>
        ) : (
          <div className="divide-y divide-zinc-800/50 border-t border-zinc-800/50">
            {detail.models.map((model, idx) => (
              <div key={idx} className="flex justify-between py-3">
                <div>
                  <div className="font-medium text-zinc-200">{model.model}</div>
                  <div className="text-xs text-zinc-500">{model.source}</div>
                </div>
                <div className="text-right">
                  <div className="text-sm text-white">
                    {formatNumber(model.tokens)}{" "}
                    <span className="text-zinc-500 text-xs">tokens</span>
                  </div>
                  {model.cost.amountMicros && model.cost.currency && (
                    <div className="text-xs text-zinc-500">
                      {formatCurrency(
                        model.cost.amountMicros,
                        model.cost.currency,
                      )}
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
