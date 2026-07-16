import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { getAccountSession, logoutAccount } from "../../ipc/client";
import { EVENT_NAMES, subscribeToEvent } from "../../ipc/events";
import type { AccountSessionResponse } from "../../ipc/generated/contracts";

const accountQueryKey = ["account", "session"] as const;

export function useAccountSession() {
  const queryClient = useQueryClient();

  useEffect(() => {
    let active = true;
    let unsubscribe: (() => void) | undefined;
    void subscribeToEvent(EVENT_NAMES.accountSessionChanged, () => {
      void queryClient.invalidateQueries({ queryKey: accountQueryKey });
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
    queryKey: accountQueryKey,
    queryFn: async () => (await getAccountSession()).data,
  });
}

export function useLogoutAccount() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async () => (await logoutAccount()).data,
    onSuccess: (session: AccountSessionResponse) => {
      queryClient.setQueryData(accountQueryKey, session);
    },
  });
}
