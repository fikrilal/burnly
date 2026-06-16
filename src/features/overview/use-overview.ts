import {
  keepPreviousData,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { useEffect, useState } from "react";

import { getUsageOverview, requestRefresh } from "../../ipc/client";
import { EVENT_NAMES, subscribeToEvent } from "../../ipc/events";
import type { UsageOverviewRequest } from "../../ipc/generated/contracts";

export function useOverview(request: UsageOverviewRequest) {
  const queryClient = useQueryClient();
  const queryKey = ["usage", "overview", request];
  const [refreshError, setRefreshError] = useState<Error | null>(null);

  const query = useQuery({
    queryKey,
    queryFn: async () => {
      const response = await getUsageOverview(request);
      return response.data;
    },
    placeholderData: keepPreviousData,
  });

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let active = true;

    const setup = async () => {
      const fn = await subscribeToEvent(EVENT_NAMES.dataInvalidated, () => {
        void queryClient.invalidateQueries({ queryKey: ["usage", "overview"] });
      });

      if (active) {
        unlisten = fn;
      } else {
        fn();
      }
    };

    void setup();

    return () => {
      active = false;
      if (unlisten) {
        unlisten();
      }
    };
  }, [queryClient]);

  const manualRefresh = async () => {
    try {
      setRefreshError(null);
      await requestRefresh();
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
  };
}
