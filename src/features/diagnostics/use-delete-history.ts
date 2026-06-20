import { useMutation, useQueryClient } from "@tanstack/react-query";

import { deleteHistory, getDeleteHistoryPreview } from "../../ipc/client";

export function useDeleteHistoryPreview() {
  return useMutation({
    mutationFn: async () => (await getDeleteHistoryPreview()).data,
  });
}

export function useDeleteHistory() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({
      previewToken,
      confirmation,
    }: {
      previewToken: string;
      confirmation: string;
    }) => (await deleteHistory(previewToken, confirmation)).data,
    onSuccess: async () => {
      await queryClient.invalidateQueries();
    },
  });
}
