import { useQuery } from "@tanstack/react-query";

import { getDiagnosticsStatus } from "../../ipc/client";

export const diagnosticsQueryKey = ["diagnostics", "status"] as const;

export function useDiagnostics() {
  return useQuery({
    queryKey: diagnosticsQueryKey,
    queryFn: async () => (await getDiagnosticsStatus()).data,
  });
}
