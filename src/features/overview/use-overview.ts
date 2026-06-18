import {
  keepPreviousData,
  type QueryClient,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { useEffect, useState } from "react";

import { getUsageOverview, requestRefresh } from "../../ipc/client";
import { EVENT_NAMES, subscribeToEvent } from "../../ipc/events";
import type {
  RefreshStatusResponse,
  UsageOverviewRequest,
} from "../../ipc/generated/contracts";

type RefreshStatus = RefreshStatusResponse["status"];

const activeRefreshStatuses = new Set<RefreshStatus>([
  "queued",
  "running",
  "cancelling",
]);

export function useOverview(request: UsageOverviewRequest) {
  const queryClient = useQueryClient();
  const queryKey = ["usage", "overview", request];
  const [refreshError, setRefreshError] = useState<Error | null>(null);
  const [refreshStatus, setRefreshStatus] = useState<RefreshStatus | null>(
    null,
  );

  const query = useQuery({
    queryKey,
    queryFn: async () => {
      const response = await getUsageOverview(request);
      return response.data;
    },
    placeholderData: keepPreviousData,
  });
  useOverviewRefreshEvents(queryClient, setRefreshStatus);

  const manualRefresh = async () => {
    try {
      setRefreshError(null);
      const response = await requestRefresh();
      setRefreshStatus(response.data.status);
    } catch (error) {
      setRefreshError(
        error instanceof Error ? error : new Error(String(error)),
      );
      console.error("Manual refresh request failed", error);
    }
  };

  return {
    ...query,
    manualRefresh,
    refreshError,
    refreshStatus,
    isRefreshing: refreshStatus
      ? activeRefreshStatuses.has(refreshStatus)
      : query.isFetching,
  };
}

function useOverviewRefreshEvents(
  queryClient: QueryClient,
  setRefreshStatus: (status: RefreshStatus) => void,
) {
  useEffect(() => {
    let unlisten: (() => void)[] = [];
    let active = true;

    void Promise.all([
      subscribeToEvent(EVENT_NAMES.dataInvalidated, () => {
        void queryClient.invalidateQueries({ queryKey: ["usage", "overview"] });
      }),
      subscribeToEvent(EVENT_NAMES.refreshProgress, (payload) => {
        const status = refreshStatusFromPayload(payload);
        if (status) setRefreshStatus(status);
      }),
    ]).then((listeners) => {
      if (active) unlisten = listeners;
      else {
        listeners.forEach((listener) => {
          listener();
        });
      }
    });

    return () => {
      active = false;
      unlisten.forEach((listener) => {
        listener();
      });
    };
  }, [queryClient, setRefreshStatus]);
}

function refreshStatusFromPayload(payload: Record<string, unknown>) {
  const status = payload.status;

  switch (status) {
    case "idle":
    case "queued":
    case "running":
    case "cancelling":
    case "succeeded":
    case "partial":
    case "failed":
      return status;
    default:
      return null;
  }
}
