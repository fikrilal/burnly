import { useState } from "react";

import type {
  ExportDataset,
  ExportPreviewRequest,
} from "../../ipc/generated/contracts";
import { useExportHistory, useExportPreview } from "./use-export";

const datasetOptions: readonly { value: ExportDataset; label: string }[] = [
  { value: "daily_usage", label: "Daily usage" },
  { value: "sessions", label: "Sessions" },
];

export function ExportCard({
  errorMessage,
}: {
  errorMessage: (error: unknown) => string;
}) {
  const [request, setRequest] = useState<ExportPreviewRequest>(initialRequest);
  const preview = useExportPreview();
  const exportMutation = useExportHistory();

  function updateRequest(next: ExportPreviewRequest) {
    setRequest(next);
    preview.reset();
    exportMutation.reset();
  }

  function toggleDataset(dataset: ExportDataset) {
    const datasets = request.datasets.includes(dataset)
      ? request.datasets.filter((value) => value !== dataset)
      : [...request.datasets, dataset];
    updateRequest({ ...request, datasets });
  }

  return (
    <section className="rounded-2xl border border-zinc-800 bg-zinc-900/60 p-5">
      <p className="text-xs uppercase tracking-wide text-zinc-500">Export</p>
      <h2 className="mt-2 text-lg font-semibold text-zinc-100">
        Export approved usage data
      </h2>
      <p className="mt-2 text-sm text-zinc-400">
        Preview the exact scope before choosing a CSV destination.
      </p>
      <div className="mt-4 grid gap-3 sm:grid-cols-2">
        <label className="text-sm text-zinc-300">
          Start date
          <input
            aria-label="Export start date"
            type="date"
            value={request.startDate}
            className={inputClass}
            onChange={(event) => {
              updateRequest({ ...request, startDate: event.target.value });
            }}
          />
        </label>
        <label className="text-sm text-zinc-300">
          End date
          <input
            aria-label="Export end date"
            type="date"
            value={request.endDate}
            className={inputClass}
            onChange={(event) => {
              updateRequest({ ...request, endDate: event.target.value });
            }}
          />
        </label>
      </div>
      <fieldset className="mt-4">
        <legend className="text-sm text-zinc-300">Datasets</legend>
        <div className="mt-2 flex flex-wrap gap-4">
          {datasetOptions.map((option) => (
            <label
              key={option.value}
              className="flex items-center gap-2 text-sm text-zinc-400"
            >
              <input
                type="checkbox"
                checked={request.datasets.includes(option.value)}
                onChange={() => {
                  toggleDataset(option.value);
                }}
              />
              {option.label}
            </label>
          ))}
        </div>
      </fieldset>
      <button
        type="button"
        className={`${buttonClass} mt-4`}
        disabled={preview.isPending || request.datasets.length === 0}
        onClick={() => {
          preview.mutate(request);
        }}
      >
        {preview.isPending ? "Preparing preview..." : "Preview export"}
      </button>
      {preview.isError ? (
        <ErrorPanel message={errorMessage(preview.error)} />
      ) : null}
      {preview.data ? (
        <div className="mt-4 rounded-xl border border-zinc-800 bg-zinc-950/50 p-4">
          <p className="text-sm font-medium text-zinc-200">
            CSV preview · {preview.data.totalRows} rows ·{" "}
            {formatBytes(preview.data.estimatedBytes)}
          </p>
          <ul className="mt-3 space-y-1 text-sm text-zinc-400">
            {preview.data.datasets.map((dataset) => (
              <li key={dataset.dataset}>
                {datasetLabel(dataset.dataset)}: {dataset.rows} rows
              </li>
            ))}
          </ul>
          <ul className="mt-3 space-y-1 text-xs text-zinc-500">
            {preview.data.privacyNotes.map((note) => (
              <li key={note}>{note}</li>
            ))}
          </ul>
          <button
            type="button"
            className={`${buttonClass} mt-4`}
            disabled={!preview.data.canExport || exportMutation.isPending}
            onClick={() => {
              exportMutation.mutate({
                request,
                previewToken: preview.data.previewToken,
              });
            }}
          >
            {exportMutation.isPending
              ? "Choosing destination..."
              : "Export CSV"}
          </button>
        </div>
      ) : null}
      {exportMutation.data ? (
        <p className="mt-4 rounded-lg bg-zinc-950/60 px-3 py-2 text-sm text-zinc-300">
          {exportMutation.data.message}
        </p>
      ) : null}
      {exportMutation.isError ? (
        <ErrorPanel message={errorMessage(exportMutation.error)} />
      ) : null}
    </section>
  );
}

function ErrorPanel({ message }: { message: string }) {
  return (
    <p className="mt-4 rounded-lg border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-red-200">
      {message}
    </p>
  );
}

function initialRequest(): ExportPreviewRequest {
  const end = new Date();
  const start = new Date(end);
  start.setUTCDate(start.getUTCDate() - 29);
  return {
    startDate: dateValue(start),
    endDate: dateValue(end),
    datasets: ["daily_usage", "sessions"],
  };
}

function dateValue(value: Date) {
  return value.toISOString().slice(0, 10);
}
function datasetLabel(value: ExportDataset) {
  return value === "daily_usage" ? "Daily usage" : "Sessions";
}
function formatBytes(value: string) {
  return `${new Intl.NumberFormat().format(BigInt(value))} estimated bytes`;
}

const inputClass =
  "mt-1 block w-full rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-2 text-zinc-100";
const buttonClass =
  "rounded-lg border border-zinc-700 px-3 py-2 text-sm font-medium text-zinc-200 transition hover:border-zinc-500 hover:bg-zinc-800 disabled:cursor-not-allowed disabled:opacity-50";
