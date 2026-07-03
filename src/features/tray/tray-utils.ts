import { formatNumber } from "../../lib/format";
import type { TraySummaryResponse } from "../../ipc/generated/contracts";
import type { FreshnessState, ModelUsage } from "../../components/burnly";

export function relativeUpdated(iso: string | null): string {
  if (!iso) return "Never updated";
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return "Updated recently";
  const minutes = Math.floor((Date.now() - then) / 60000);
  if (minutes < 1) return "Updated just now";
  if (minutes < 60) return `Updated ${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `Updated ${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `Updated ${days}d ago`;
}

export function freshnessState(
  dataStatus: TraySummaryResponse["dataStatus"],
  isRefreshing: boolean,
  isError: boolean,
): FreshnessState {
  if (isError) return "failed";
  if (isRefreshing) return "refreshing";
  return dataStatus;
}

export function toModelUsage(
  models: TraySummaryResponse["models"],
): ModelUsage[] {
  return models.map((model) => ({
    modelName: model.modelName,
    agentLabel: model.agentLabel,
    tokens: formatNumber(model.totalTokens),
    trend: model.trend,
  }));
}

export function tokenNumber(value: string): number {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : 0;
}
