import { useState, type ReactNode } from "react";

import type { DatabaseMaintenanceActionResponse } from "../../ipc/generated/contracts";
import { useDatabaseMaintenance } from "./use-database-maintenance";

export function DatabaseMaintenanceCard({
  errorMessage,
}: {
  errorMessage: (error: unknown) => string;
}) {
  const maintenance = useDatabaseMaintenance();
  const [vacuumArmed, setVacuumArmed] = useState(false);
  const [restoreArmed, setRestoreArmed] = useState(false);
  const status = maintenance.status.data;
  const active =
    maintenance.integrity.isPending ||
    maintenance.checkpoint.isPending ||
    maintenance.vacuum.isPending ||
    maintenance.restore.isPending;
  const result =
    maintenance.restore.data ??
    maintenance.vacuum.data ??
    maintenance.checkpoint.data ??
    maintenance.integrity.data;
  const error =
    maintenance.restore.error ??
    maintenance.vacuum.error ??
    maintenance.checkpoint.error ??
    maintenance.integrity.error;

  return (
    <section className="rounded-2xl border border-zinc-800 bg-zinc-900/60 p-5">
      <p className="text-xs uppercase tracking-wide text-zinc-500">Database</p>
      <h2 className="mt-2 text-lg font-semibold text-zinc-100">
        Maintenance and recovery
      </h2>
      <p className="mt-2 text-sm text-zinc-400">
        Run explicit SQLite maintenance only while refresh work is idle.
      </p>

      {maintenance.status.isPending ? (
        <p className="mt-4 text-sm text-zinc-400">Loading database status...</p>
      ) : null}
      {maintenance.status.isError ? (
        <Message tone="error">{errorMessage(maintenance.status.error)}</Message>
      ) : null}
      {status ? (
        <dl className="mt-4 grid gap-3 rounded-xl bg-zinc-950/50 p-4 text-sm sm:grid-cols-3">
          <StatusItem label="Access" value={accessLabel(status.access)} />
          <StatusItem
            label="Schema"
            value={
              status.schemaVersion === null
                ? "Unknown"
                : `v${status.schemaVersion}`
            }
          />
          <StatusItem
            label="Recovery backup"
            value={status.backupAvailable ? "Available" : "Not available"}
          />
        </dl>
      ) : null}

      {status?.access === "read_only" ? (
        <Message tone="warning">
          Burnly can inspect this database but cannot checkpoint or vacuum it.
          Correct the file permissions before retrying.
        </Message>
      ) : null}
      {status?.access === "unavailable" ? (
        <Message tone="error">
          The database cannot be opened. Use a verified migration backup when
          one is available.
        </Message>
      ) : null}

      <div className="mt-4 flex flex-wrap gap-3">
        <ActionButton
          disabled={active || !status}
          label={
            maintenance.integrity.isPending ? "Checking..." : "Check integrity"
          }
          onClick={() => {
            maintenance.integrity.mutate();
          }}
        />
        <ActionButton
          disabled={active || !status?.maintenanceAvailable}
          label={
            maintenance.checkpoint.isPending
              ? "Checkpointing..."
              : "Checkpoint WAL"
          }
          onClick={() => {
            maintenance.checkpoint.mutate();
          }}
        />
        <ActionButton
          disabled={active || !status?.maintenanceAvailable}
          label={
            vacuumArmed
              ? "Confirm vacuum"
              : maintenance.vacuum.isPending
                ? "Vacuuming..."
                : "Vacuum database"
          }
          onClick={() => {
            if (!vacuumArmed) {
              setVacuumArmed(true);
              return;
            }
            setVacuumArmed(false);
            maintenance.vacuum.mutate();
          }}
        />
        {status?.backupAvailable ? (
          <ActionButton
            disabled={active}
            label={restoreArmed ? "Confirm restore" : "Restore backup"}
            danger
            onClick={() => {
              if (!restoreArmed) {
                setRestoreArmed(true);
                return;
              }
              setRestoreArmed(false);
              maintenance.restore.mutate();
            }}
          />
        ) : null}
      </div>

      {result ? <ResultMessage result={result} /> : null}
      {error ? <Message tone="error">{errorMessage(error)}</Message> : null}
    </section>
  );
}

function StatusItem({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-zinc-500">{label}</dt>
      <dd className="mt-1 text-zinc-200">{value}</dd>
    </div>
  );
}

function ActionButton({
  disabled,
  label,
  onClick,
  danger = false,
}: {
  disabled: boolean;
  label: string;
  onClick: () => void;
  danger?: boolean;
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      className={`rounded-lg border px-3 py-2 text-sm font-medium transition disabled:cursor-not-allowed disabled:opacity-50 ${
        danger
          ? "border-red-800 text-red-200 hover:bg-red-950/50"
          : "border-zinc-700 text-zinc-200 hover:border-zinc-500 hover:bg-zinc-800"
      }`}
      onClick={onClick}
    >
      {label}
    </button>
  );
}

function ResultMessage({
  result,
}: {
  result: DatabaseMaintenanceActionResponse;
}) {
  const details = result.checkpoint
    ? ` ${result.checkpoint.checkpointedFrames} of ${result.checkpoint.logFrames} WAL frames processed.`
    : "";
  return (
    <Message tone={result.status === "corrupt" ? "error" : "success"}>
      {result.message}
      {details}
    </Message>
  );
}

function Message({
  children,
  tone,
}: {
  children: ReactNode;
  tone: "success" | "warning" | "error";
}) {
  const className = {
    success: "border-emerald-500/30 bg-emerald-500/10 text-emerald-200",
    warning: "border-amber-500/30 bg-amber-500/10 text-amber-200",
    error: "border-red-500/30 bg-red-500/10 text-red-200",
  }[tone];
  return (
    <p className={`mt-4 rounded-lg border px-3 py-2 text-sm ${className}`}>
      {children}
    </p>
  );
}

function accessLabel(access: "read_write" | "read_only" | "unavailable") {
  return {
    read_write: "Read/write",
    read_only: "Read-only",
    unavailable: "Unavailable",
  }[access];
}
