import { useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";

import type { DeleteHistoryPreviewResponse } from "../../ipc/generated/contracts";
import {
  useDeleteHistory,
  useDeleteHistoryPreview,
} from "./use-delete-history";

export function DeleteHistoryCard({
  errorMessage,
}: {
  errorMessage: (error: unknown) => string;
}) {
  const preview = useDeleteHistoryPreview();
  const deletion = useDeleteHistory();
  const [dialogOpen, setDialogOpen] = useState(false);
  const [confirmation, setConfirmation] = useState("");

  function closeDialog() {
    setDialogOpen(false);
    setConfirmation("");
  }

  return (
    <section className="rounded-2xl border border-red-950/70 bg-zinc-900/60 p-5">
      <p className="text-xs uppercase tracking-wide text-red-400">
        Destructive maintenance
      </p>
      <h2 className="mt-2 text-lg font-semibold text-zinc-100">
        Delete imported history
      </h2>
      <p className="mt-2 text-sm text-zinc-400">
        Permanently remove all imported usage and operational history while
        preserving configuration.
      </p>
      <button
        type="button"
        className={`${buttonClass} mt-4`}
        disabled={preview.isPending}
        onClick={() => {
          preview.mutate();
          deletion.reset();
        }}
      >
        {preview.isPending ? "Preparing preview..." : "Preview deletion"}
      </button>
      {preview.isError ? (
        <ErrorPanel message={errorMessage(preview.error)} />
      ) : null}
      {preview.data ? (
        <DeletionPreview
          preview={preview.data}
          onDelete={() => {
            setDialogOpen(true);
            deletion.reset();
          }}
        />
      ) : null}
      {deletion.data ? (
        <p className="mt-4 rounded-lg bg-zinc-950/60 px-3 py-2 text-sm text-zinc-300">
          {deletion.data.message} {deletion.data.deletedRecords} records
          removed.
        </p>
      ) : null}
      {deletion.isError ? (
        <ErrorPanel message={errorMessage(deletion.error)} />
      ) : null}

      <Dialog.Root
        open={dialogOpen}
        onOpenChange={(open) => {
          if (!open) closeDialog();
        }}
      >
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 bg-black/70" />
          <Dialog.Content className="fixed left-1/2 top-1/2 w-[min(92vw,32rem)] -translate-x-1/2 -translate-y-1/2 rounded-2xl border border-red-900 bg-zinc-950 p-6 shadow-2xl">
            <Dialog.Title className="text-xl font-semibold text-zinc-50">
              Delete all imported history?
            </Dialog.Title>
            <Dialog.Description className="mt-2 text-sm text-zinc-400">
              This cannot be undone. Type{" "}
              <strong className="text-zinc-200">
                {preview.data?.confirmationText}
              </strong>{" "}
              to continue.
            </Dialog.Description>
            <input
              aria-label="Delete history confirmation"
              className={`${inputClass} mt-4`}
              value={confirmation}
              onChange={(event) => {
                setConfirmation(event.target.value);
              }}
            />
            <div className="mt-5 flex justify-end gap-3">
              <button
                type="button"
                className={buttonClass}
                onClick={closeDialog}
              >
                Cancel
              </button>
              <button
                type="button"
                className={dangerButtonClass}
                disabled={
                  deletion.isPending ||
                  confirmation !== preview.data?.confirmationText
                }
                onClick={() => {
                  if (!preview.data) return;
                  deletion.mutate(
                    { previewToken: preview.data.previewToken, confirmation },
                    { onSuccess: closeDialog },
                  );
                }}
              >
                {deletion.isPending ? "Deleting..." : "Delete all history"}
              </button>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
    </section>
  );
}

function DeletionPreview({
  preview,
  onDelete,
}: {
  preview: DeleteHistoryPreviewResponse;
  onDelete: () => void;
}) {
  return (
    <div className="mt-4 rounded-xl border border-red-900/60 bg-red-950/20 p-4">
      <p className="text-sm font-medium text-red-100">
        {preview.totalRecords} records across {preview.sourceCount} sources
      </p>
      <p className="mt-1 text-sm text-zinc-400">{dateScope(preview)}</p>
      <ul className="mt-3 grid gap-1 text-sm text-zinc-400 sm:grid-cols-2">
        <li>Daily usage: {preview.counts.dailyUsage}</li>
        <li>Sessions: {preview.counts.sessions}</li>
        <li>Refresh runs: {preview.counts.refreshRuns}</li>
        <li>Import runs: {preview.counts.importRuns}</li>
      </ul>
      <p className="mt-3 text-xs font-medium uppercase tracking-wide text-zinc-500">
        Preserved
      </p>
      <ul className="mt-2 space-y-1 text-sm text-zinc-400">
        {preview.preserved.map((item) => (
          <li key={item}>{item}</li>
        ))}
      </ul>
      {preview.activeRefresh ? (
        <p className="mt-3 text-sm text-amber-300">
          Wait for the active refresh to finish.
        </p>
      ) : null}
      <button
        type="button"
        className={`${dangerButtonClass} mt-4`}
        disabled={!preview.canDelete}
        onClick={onDelete}
      >
        Delete history
      </button>
    </div>
  );
}

function ErrorPanel({ message }: { message: string }) {
  return (
    <p className="mt-4 rounded-lg border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-red-200">
      {message}
    </p>
  );
}
function dateScope(preview: DeleteHistoryPreviewResponse) {
  return preview.earliestDate && preview.latestDate
    ? `${preview.earliestDate} through ${preview.latestDate}`
    : "No dated usage records";
}

const inputClass =
  "block w-full rounded-lg border border-zinc-700 bg-zinc-900 px-3 py-2 text-zinc-100";
const buttonClass =
  "rounded-lg border border-zinc-700 px-3 py-2 text-sm font-medium text-zinc-200 transition hover:border-zinc-500 hover:bg-zinc-800 disabled:cursor-not-allowed disabled:opacity-50";
const dangerButtonClass =
  "rounded-lg bg-red-700 px-3 py-2 text-sm font-medium text-white transition hover:bg-red-600 disabled:cursor-not-allowed disabled:opacity-50";
