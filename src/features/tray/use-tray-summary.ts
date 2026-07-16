import {
  keepPreviousData,
  type QueryClient,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { useEffect, useState } from "react";

import { getTraySummary } from "../../ipc/client";
import { EVENT_NAMES, subscribeToEvent } from "../../ipc/events";
import type { RefreshStatusResponse } from "../../ipc/generated/contracts";

type RefreshStatus = RefreshStatusResponse["status"];

const activeRefreshStatuses = new Set<RefreshStatus>([
  "queued",
  "running",
  "cancelling",
]);

export function useTraySummary(reportingTimezone: string) {
  const queryClient = useQueryClient();
  const [refreshStatus, setRefreshStatus] = useState<RefreshStatus | null>(
    null,
  );
  const queryKey = ["usage", "tray-summary", reportingTimezone];

  const query = useQuery({
    queryKey,
    queryFn: async () => {
      const response = await getTraySummary({ reportingTimezone });
      return response.data;
    },
    placeholderData: keepPreviousData,
  });

  useTraySummaryEvents(queryClient, setRefreshStatus);

  return {
    ...query,
    refreshStatus,
    isRefreshing: refreshStatus
      ? activeRefreshStatuses.has(refreshStatus)
      : query.isFetching,
  };
}

function useTraySummaryEvents(
  queryClient: QueryClient,
  setRefreshStatus: (status: RefreshStatus) => void,
) {
  useEffect(() => {
    let unlisten: (() => void)[] = [];
    let active = true;

    void Promise.all([
      subscribeToEvent(EVENT_NAMES.dataInvalidated, () => {
        void queryClient.invalidateQueries({
          queryKey: ["usage", "tray-summary"],
        });
      }),
      subscribeToEvent(EVENT_NAMES.refreshProgress, (payload) => {
        const status = refreshStatusFromPayload(payload);
        if (status) setRefreshStatus(status);
      }),
    ]).then((listeners) => {
      if (active) {
        unlisten = listeners;
      } else {
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

function refreshStatusFromPayload(payload: { status: string }) {
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
