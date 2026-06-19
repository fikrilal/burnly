import { useMutation, useQuery } from "@tanstack/react-query";

import { getDiagnosticsStatus, revealDiagnosticsLogs } from "../../ipc/client";

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
