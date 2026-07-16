import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { getCollectSyncStatus, retryCollectSync } from "../../ipc/client";
import { EVENT_NAMES, subscribeToEvent } from "../../ipc/events";
import type { CollectSyncStatusResponse } from "../../ipc/generated/contracts";

const collectSyncQueryKey = ["collect-sync", "status"] as const;

export function useCollectSyncStatus(enabled: boolean) {
  const queryClient = useQueryClient();

  useEffect(() => {
    if (!enabled) {
      return;
    }
    let active = true;
    let unsubscribe: (() => void) | undefined;
    void subscribeToEvent(EVENT_NAMES.collectSyncChanged, () => {
      void queryClient.invalidateQueries({ queryKey: collectSyncQueryKey });
    }).then((listener) => {
      if (active) {
        unsubscribe = listener;
      } else {
        listener();
      }
    });
    return () => {
      active = false;
      unsubscribe?.();
    };
  }, [enabled, queryClient]);

  return useQuery({
    queryKey: collectSyncQueryKey,
    queryFn: async () => (await getCollectSyncStatus()).data,
    enabled,
  });
}

export function useRetryCollectSync() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async () => (await retryCollectSync()).data,
    onSuccess: (status: CollectSyncStatusResponse) => {
      queryClient.setQueryData(collectSyncQueryKey, status);
    },
  });
}
