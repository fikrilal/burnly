import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  checkForUpdate,
  downloadUpdate,
  getUpdateState,
  restartForUpdate,
} from "../../ipc/client";
import { EVENT_NAMES, subscribeToEvent } from "../../ipc/events";
import type { UpdateStatusResponse } from "../../ipc/generated/contracts";

const updateQueryKey = ["updateState"] as const;

export function useUpdateState() {
  const queryClient = useQueryClient();

  useEffect(() => {
    let active = true;
    let unsubscribe: (() => void) | undefined;
    void subscribeToEvent(EVENT_NAMES.updateProgress, () => {
      void queryClient.invalidateQueries({ queryKey: updateQueryKey });
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
  }, [queryClient]);

  return useQuery({
    queryKey: updateQueryKey,
    queryFn: async () => (await getUpdateState()).data,
  });
}

export function useCheckForUpdate() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async () => (await checkForUpdate()).data,
    onSuccess: (data: UpdateStatusResponse) => {
      queryClient.setQueryData(updateQueryKey, data);
    },
  });
}

export function useDownloadUpdate() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async () => (await downloadUpdate()).data,
    onSuccess: (data: UpdateStatusResponse) => {
      queryClient.setQueryData(updateQueryKey, data);
    },
  });
}

export function useRestartForUpdate() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async () => (await restartForUpdate()).data,
    onSuccess: (data: UpdateStatusResponse) => {
      queryClient.setQueryData(updateQueryKey, data);
    },
  });
}
