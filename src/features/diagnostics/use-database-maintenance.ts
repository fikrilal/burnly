import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  checkDatabaseIntegrity,
  checkpointDatabase,
  getDatabaseMaintenanceStatus,
  restoreDatabaseMigrationBackup,
  vacuumDatabase,
} from "../../ipc/client";

export const databaseMaintenanceQueryKey = [
  "diagnostics",
  "database-maintenance",
] as const;

export function useDatabaseMaintenance() {
  const queryClient = useQueryClient();
  const status = useQuery({
    queryKey: databaseMaintenanceQueryKey,
    queryFn: async () => (await getDatabaseMaintenanceStatus()).data,
  });
  const invalidateStatus = async () => {
    await queryClient.invalidateQueries({
      queryKey: databaseMaintenanceQueryKey,
    });
  };

  return {
    status,
    integrity: useMutation({
      mutationFn: async () => (await checkDatabaseIntegrity()).data,
      onSuccess: invalidateStatus,
    }),
    checkpoint: useMutation({
      mutationFn: async () => (await checkpointDatabase()).data,
      onSuccess: invalidateStatus,
    }),
    vacuum: useMutation({
      mutationFn: async () => (await vacuumDatabase()).data,
      onSuccess: invalidateStatus,
    }),
    restore: useMutation({
      mutationFn: async () => (await restoreDatabaseMigrationBackup()).data,
      onSuccess: invalidateStatus,
    }),
  };
}
