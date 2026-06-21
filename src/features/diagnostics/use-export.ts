import { useMutation } from "@tanstack/react-query";

import { exportHistory, getExportPreview } from "../../ipc/client";
import type { ExportPreviewRequest } from "../../ipc/generated/contracts";

export function useExportPreview() {
  return useMutation({
    mutationFn: async (request: ExportPreviewRequest) =>
      (await getExportPreview(request)).data,
  });
}

export function useExportHistory() {
  return useMutation({
    mutationFn: async ({
      request,
      previewToken,
    }: {
      request: ExportPreviewRequest;
      previewToken: string;
    }) => (await exportHistory(request, previewToken)).data,
  });
}
