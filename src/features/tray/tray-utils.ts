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

export interface TrayHeaderStatusInput {
  dataStatus: TraySummaryResponse["dataStatus"];
  dataQuality: TraySummaryResponse["dataQuality"];
  latestRefreshStatus: TraySummaryResponse["latestRefreshStatus"];
  isRefreshing: boolean;
  isError: boolean;
}

export function trayHeaderStatus(input: TrayHeaderStatusInput): FreshnessState {
  if (input.isError) return "failed";
  if (input.isRefreshing) return "refreshing";
  if (
    input.latestRefreshStatus === "failed" ||
    input.latestRefreshStatus === "cancelled"
  ) {
    return "failed";
  }
  if (input.latestRefreshStatus === "partial") return "partial";
  if (input.dataQuality === "partial") return "estimated";
  return input.dataStatus;
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
