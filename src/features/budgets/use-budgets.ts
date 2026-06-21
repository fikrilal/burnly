import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  createBudget,
  deleteBudget,
  disableBudget,
  enableBudget,
  listBudgets,
  updateBudget,
} from "../../ipc/client";
import { EVENT_NAMES, subscribeToEvent } from "../../ipc/events";
import type {
  BudgetListResponse,
  BudgetResponse,
  CreateBudgetRequest,
  DeleteBudgetResponse,
  MutateBudgetRequest,
  UpdateBudgetRequest,
} from "../../ipc/generated/contracts";

export const budgetsQueryKey = ["budgets"] as const;
const budgetProgressQueryKey = ["budgets", "progress"] as const;

export function useBudgets() {
  const queryClient = useQueryClient();

  useEffect(() => {
    let active = true;
    let unsubscribe: (() => void) | undefined;

    void subscribeToEvent(EVENT_NAMES.dataInvalidated, (payload) => {
      if (payload.scope === "budgets") {
        void queryClient.invalidateQueries({ queryKey: budgetsQueryKey });
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

  return useQuery({
    queryKey: budgetsQueryKey,
    queryFn: async () => (await listBudgets()).data,
  });
}

export function useCreateBudget() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (request: CreateBudgetRequest) =>
      (await createBudget(request)).data,
    onSuccess: (budget) => {
      upsertBudget(queryClient, budget);
      invalidateBudgetProgress(queryClient);
    },
  });
}

export function useUpdateBudget() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (request: UpdateBudgetRequest) =>
      (await updateBudget(request)).data,
    onSuccess: (budget) => {
      upsertBudget(queryClient, budget);
      invalidateBudgetProgress(queryClient);
    },
  });
}

export function useEnableBudget() {
  return useBudgetMutation(enableBudget);
}

export function useDisableBudget() {
  return useBudgetMutation(disableBudget);
}

export function useDeleteBudget() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (request: MutateBudgetRequest) =>
      (await deleteBudget(request)).data,
    onSuccess: (result) => {
      removeBudget(queryClient, result);
      invalidateBudgetProgress(queryClient);
    },
  });
}

function useBudgetMutation(
  mutation: (request: MutateBudgetRequest) => Promise<{ data: BudgetResponse }>,
) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (request: MutateBudgetRequest) =>
      (await mutation(request)).data,
    onSuccess: (budget) => {
      upsertBudget(queryClient, budget);
      invalidateBudgetProgress(queryClient);
    },
  });
}

function invalidateBudgetProgress(
  queryClient: ReturnType<typeof useQueryClient>,
) {
  void queryClient.invalidateQueries({ queryKey: budgetProgressQueryKey });
}

function upsertBudget(
  queryClient: ReturnType<typeof useQueryClient>,
  budget: BudgetResponse,
) {
  queryClient.setQueryData<BudgetListResponse>(budgetsQueryKey, (current) => {
    if (!current) return { items: [budget] };

    const exists = current.items.some((item) => item.id === budget.id);
    const items = exists
      ? current.items.map((item) => (item.id === budget.id ? budget : item))
      : [budget, ...current.items];

    return { items };
  });
}

function removeBudget(
  queryClient: ReturnType<typeof useQueryClient>,
  result: DeleteBudgetResponse,
) {
  queryClient.setQueryData<BudgetListResponse>(budgetsQueryKey, (current) => {
    if (!current) return current;
    return {
      items: current.items.filter((item) => item.id !== result.budgetId),
    };
  });
}
