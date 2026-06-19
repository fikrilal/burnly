import {
  keepPreviousData,
  type QueryClient,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { useEffect } from "react";

import { getCurrentBudgetProgress } from "../../ipc/client";
import { EVENT_NAMES, subscribeToEvent } from "../../ipc/events";

export const budgetProgressQueryKey = ["budgets", "progress"] as const;

export function useBudgetProgress() {
  const queryClient = useQueryClient();
  useBudgetProgressInvalidation(queryClient);

  return useQuery({
    queryKey: budgetProgressQueryKey,
    queryFn: async () => (await getCurrentBudgetProgress()).data,
    placeholderData: keepPreviousData,
  });
}

function useBudgetProgressInvalidation(queryClient: QueryClient) {
  useEffect(() => {
    let active = true;
    let unsubscribe: (() => void) | undefined;

    void subscribeToEvent(EVENT_NAMES.dataInvalidated, (payload) => {
      if (payload.scope === "usage" || payload.scope === "budgets") {
        void queryClient.invalidateQueries({
          queryKey: budgetProgressQueryKey,
        });
      }
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
}
