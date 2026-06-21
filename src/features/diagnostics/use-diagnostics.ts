import { useInfiniteQuery, useMutation, useQuery } from "@tanstack/react-query";

import {
  getDiagnosticsHistory,
  getDiagnosticsStatus,
  revealDiagnosticsLogs,
} from "../../ipc/client";

export const diagnosticsQueryKey = ["diagnostics", "status"] as const;

export function useDiagnostics() {
  return useQuery({
    queryKey: diagnosticsQueryKey,
    queryFn: async () => (await getDiagnosticsStatus()).data,
  });
}

export function useRevealDiagnosticsLogs() {
  return useMutation({
    mutationFn: async () => (await revealDiagnosticsLogs()).data,
  });
}

export const diagnosticsHistoryQueryKey = ["diagnostics", "history"] as const;

export function useDiagnosticsHistory() {
  return useInfiniteQuery({
    queryKey: diagnosticsHistoryQueryKey,
    initialPageParam: null as string | null,
    queryFn: async ({ pageParam }) =>
      (
        await getDiagnosticsHistory(
          pageParam ? { cursor: pageParam, limit: 10 } : { limit: 10 },
        )
      ).data,
    getNextPageParam: (lastPage) => lastPage.nextCursor,
  });
}
