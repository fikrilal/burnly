import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { getSettings, updateSettings } from "../../ipc/client";
import { EVENT_NAMES, subscribeToEvent } from "../../ipc/events";
import type {
  SettingsResponse,
  UpdateSettingsRequest,
} from "../../ipc/generated/contracts";

const settingsQueryKey = ["settings"] as const;

export function useSettings() {
  const queryClient = useQueryClient();
  useEffect(() => {
    let active = true;
    let unsubscribe: (() => void) | undefined;
    void subscribeToEvent(EVENT_NAMES.settingsChanged, () => {
      void queryClient.invalidateQueries({ queryKey: settingsQueryKey });
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
    queryKey: settingsQueryKey,
    queryFn: async () => (await getSettings()).data,
  });
}

export function useUpdateSettings() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (request: UpdateSettingsRequest) =>
      (await updateSettings(request)).data,
    onSuccess: (settings: SettingsResponse) => {
      queryClient.setQueryData(settingsQueryKey, settings);
    },
  });
}
