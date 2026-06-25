import { formatNumber } from "../../../lib/format";

interface PrimaryMetricProps {
  totalTokens: string;
}

export function PrimaryMetric({ totalTokens }: PrimaryMetricProps) {
  return (
    <div>
      <p className="text-sm font-medium text-zinc-400">Today token usage</p>
      <p className="mt-2 text-5xl font-semibold tracking-tight text-white">
        {formatNumber(totalTokens)}
      </p>
      <p className="mt-1 text-xs text-zinc-500">tokens today</p>
    </div>
  );
}

interface SecondaryMetricRowProps {
  weekTokens: string;
  monthTokens: string;
}

export function SecondaryMetricRow({
  weekTokens,
  monthTokens,
}: SecondaryMetricRowProps) {
  return (
    <div className="grid grid-cols-2 gap-3">
      <SecondaryMetric label="This week" totalTokens={weekTokens} />
      <SecondaryMetric label="This month" totalTokens={monthTokens} />
    </div>
  );
}

function SecondaryMetric({
  label,
  totalTokens,
}: {
  label: string;
  totalTokens: string;
}) {
  return (
    <div className="rounded-xl border border-zinc-800 bg-zinc-950/50 p-3">
      <p className="text-xs text-zinc-500">{label}</p>
      <p className="mt-1 text-lg font-semibold text-zinc-100">
        {formatNumber(totalTokens)}
      </p>
    </div>
  );
}
