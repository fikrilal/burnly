import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  cancelAccountLogin,
  getAccountSession,
  logoutAccount,
  startAccountLogin,
} from "../../ipc/client";
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

function useAccountMutation(
  mutationFn: () => Promise<AccountSessionResponse>,
) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn,
    onSuccess: (session: AccountSessionResponse) => {
      queryClient.setQueryData(accountQueryKey, session);
    },
  });
}

export function useStartAccountLogin() {
  return useAccountMutation(async () => (await startAccountLogin()).data);
}

export function useCancelAccountLogin() {
  return useAccountMutation(async () => (await cancelAccountLogin()).data);
}

export function useLogoutAccount() {
  return useAccountMutation(async () => (await logoutAccount()).data);
}
