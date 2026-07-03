import { useMutation, useQuery } from "@tanstack/react-query";

import {
  copyDiagnosticsReport,
  exportDiagnosticsReport,
  getDiagnosticsHealth,
} from "../../ipc/client";

const diagnosticsHealthQueryKey = ["diagnosticsHealth"] as const;

export function useDiagnosticsHealth() {
  return useQuery({
    queryKey: diagnosticsHealthQueryKey,
    queryFn: async () => (await getDiagnosticsHealth()).data,
  });
}

export function useExportDiagnosticsReport() {
  return useMutation({
    mutationFn: async () => (await exportDiagnosticsReport()).data,
  });
}

export function useCopyDiagnosticsReport() {
  return useMutation({
    mutationFn: async () => (await copyDiagnosticsReport()).data,
  });
}
