import type { ReactNode } from "react";
import { AlertTriangle, CheckCircle2, CircleHelp, XCircle } from "lucide-react";

import { BurnlyClientError } from "../../ipc/errors";
import type {
  DiagnosticComponentResponse,
  DiagnosticHealthStatus,
  DiagnosticsStatusResponse,
  RevealLogsResponse,
  RefreshHistoryItem,
} from "../../ipc/generated/contracts";
import {
  useDiagnostics,
  useDiagnosticsHistory,
  useRevealDiagnosticsLogs,
} from "./use-diagnostics";
import { ExportCard } from "./ExportCard";
import { DeleteHistoryCard } from "./DeleteHistoryCard";

export function DiagnosticsView() {
  const query = useDiagnostics();
  const revealLogs = useRevealDiagnosticsLogs();
  const history = useDiagnosticsHistory();

  if (query.isPending) {
    return <DiagnosticsShell title="Loading diagnostics" />;
  }

  if (query.isError) {
    return (
      <DiagnosticsShell
        title="Diagnostics unavailable"
        detail={errorMessage(query.error)}
        action={
          <button
            type="button"
            className={secondaryButtonClass}
            onClick={() => {
              void query.refetch();
            }}
          >
            Retry
          </button>
        }
      />
    );
  }

  if (query.data.components.length === 0) {
    return (
      <DiagnosticsShell
        title="No diagnostics reported"
        detail="The runtime did not return any diagnostic components."
      />
    );
  }

  return (
    <section className="space-y-5">
      <div className="rounded-2xl border border-zinc-800 bg-zinc-900/70 p-6">
        <p className="text-sm uppercase tracking-wide text-zinc-500">
          Diagnostics
        </p>
        <div className="mt-3 flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
          <div>
            <h1 className="text-3xl font-semibold text-zinc-50">
              Runtime health
            </h1>
            <p className="mt-2 text-sm text-zinc-400">
              Safe status summaries for local runtime, storage, sources, and
              collector state.
            </p>
          </div>
          <StatusBadge status={query.data.status} />
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        {query.data.components.map((component) => (
          <DiagnosticCard key={component.component} component={component} />
        ))}
      </div>

      <LogRevealCard
        logs={query.data.logs}
        result={revealLogs.data}
        error={revealLogs.error}
        isPending={revealLogs.isPending}
        onReveal={() => {
          revealLogs.mutate();
        }}
      />
      <HistorySection query={history} />
      <ExportCard errorMessage={errorMessage} />
      <DeleteHistoryCard errorMessage={errorMessage} />
    </section>
  );
}

function HistorySection({
  query,
}: {
  query: ReturnType<typeof useDiagnosticsHistory>;
}) {
  const items = query.data?.pages.flatMap((page) => page.items) ?? [];
  return (
    <section className="rounded-2xl border border-zinc-800 bg-zinc-900/60 p-5">
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="text-xs uppercase tracking-wide text-zinc-500">
            History
          </p>
          <h2 className="mt-2 text-lg font-semibold text-zinc-100">
            Import and refresh runs
          </h2>
          <p className="mt-2 text-sm text-zinc-400">
            Persisted operational summaries without paths, prompts, or session
            identifiers.
          </p>
        </div>
      </div>
      {query.isPending ? (
        <p className="mt-4 text-sm text-zinc-400">Loading run history...</p>
      ) : null}
      {query.isError ? (
        <div className="mt-4 rounded-lg border border-red-500/40 bg-red-500/10 p-3">
          <p className="text-sm text-red-200">{errorMessage(query.error)}</p>
          <button
            type="button"
            className={`${secondaryButtonClass} mt-3`}
            onClick={() => void query.refetch()}
          >
            Retry history
          </button>
        </div>
      ) : null}
      {!query.isPending && !query.isError && items.length === 0 ? (
        <p className="mt-4 rounded-lg bg-zinc-950/60 px-3 py-4 text-sm text-zinc-400">
          No import or refresh runs have been recorded.
        </p>
      ) : null}
      {items.length > 0 ? (
        <ol className="mt-4 space-y-3">
          {items.map((item, index) => (
            <HistoryCard key={`${item.startedAt}-${index}`} item={item} />
          ))}
        </ol>
      ) : null}
      {query.hasNextPage ? (
        <button
          type="button"
          className={`${secondaryButtonClass} mt-4`}
          disabled={query.isFetchingNextPage}
          onClick={() => void query.fetchNextPage()}
        >
          {query.isFetchingNextPage ? "Loading..." : "Load older runs"}
        </button>
      ) : null}
    </section>
  );
}

function HistoryCard({ item }: { item: RefreshHistoryItem }) {
  return (
    <li className="rounded-xl border border-zinc-800 bg-zinc-950/50 p-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <p className="text-sm font-medium capitalize text-zinc-200">
            {triggerLabel(item.trigger)} refresh
          </p>
          <p className="mt-1 text-xs text-zinc-500">
            {formatTimestamp(item.startedAt)}
          </p>
        </div>
        <HistoryStatusBadge status={item.status} />
      </div>
      <p className="mt-3 text-sm text-zinc-400">{item.summary}</p>
      {item.failure ? (
        <p className="mt-3 rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-sm text-amber-200">
          {failureLabel(item.failure.category)} · {item.failure.summary}
          {item.failure.retryable ? " Retry is available." : ""}
        </p>
      ) : null}
      {item.imports.length > 0 ? (
        <ul className="mt-3 divide-y divide-zinc-800 border-t border-zinc-800">
          {item.imports.map((entry, index) => (
            <li
              key={`${entry.source}-${entry.projection}-${index}`}
              className="flex flex-wrap items-center justify-between gap-2 py-3 text-sm"
            >
              <span className="text-zinc-300">
                {entry.source} · {entry.projection} · {entry.scope}
              </span>
              <span className="text-zinc-500">
                {entry.recordsSeen} accepted · {entry.recordsRejected} rejected
                · {entry.status}
              </span>
            </li>
          ))}
        </ul>
      ) : null}
    </li>
  );
}

function HistoryStatusBadge({
  status,
}: {
  status: RefreshHistoryItem["status"];
}) {
  const className =
    status === "succeeded"
      ? "border-emerald-500/40 text-emerald-300"
      : status === "running" || status === "queued"
        ? "border-blue-500/40 text-blue-300"
        : status === "failed" || status === "stale"
          ? "border-red-500/40 text-red-300"
          : "border-amber-500/40 text-amber-300";
  return (
    <span
      className={`rounded-full border px-2.5 py-1 text-xs capitalize ${className}`}
    >
      {status}
    </span>
  );
}

function triggerLabel(trigger: RefreshHistoryItem["trigger"]) {
  return trigger.replace("_", " ");
}
function failureLabel(
  category: NonNullable<RefreshHistoryItem["failure"]>["category"],
) {
  return category === "unknown"
    ? "Operational failure"
    : `${category.charAt(0).toUpperCase()}${category.slice(1)} failure`;
}
function formatTimestamp(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}

function LogRevealCard({
  logs,
  result,
  error,
  isPending,
  onReveal,
}: {
  logs: DiagnosticsStatusResponse["logs"];
  result: RevealLogsResponse | undefined;
  error: Error | null;
  isPending: boolean;
  onReveal: () => void;
}) {
  const isAvailable = logs.status === "available";

  return (
    <article className="rounded-2xl border border-zinc-800 bg-zinc-900/60 p-5">
      <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <p className="text-xs uppercase tracking-wide text-zinc-500">Logs</p>
          <h2 className="mt-2 text-lg font-semibold text-zinc-100">
            {logs.label}
          </h2>
          <p className="mt-2 text-sm text-zinc-400">
            {logStatusText(logs.status)}
          </p>
        </div>
        <button
          type="button"
          className={secondaryButtonClass}
          disabled={!isAvailable || isPending}
          onClick={onReveal}
        >
          {isPending ? "Opening..." : "Reveal logs"}
        </button>
      </div>
      {result ? (
        <p className="mt-4 rounded-lg bg-zinc-950/60 px-3 py-2 text-sm text-zinc-300">
          {result.message}
        </p>
      ) : null}
      {error ? (
        <p className="mt-4 rounded-lg border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-red-200">
          {errorMessage(error)}
        </p>
      ) : null}
    </article>
  );
}

function DiagnosticCard({
  component,
}: {
  component: DiagnosticComponentResponse;
}) {
  return (
    <article className="rounded-2xl border border-zinc-800 bg-zinc-900/60 p-5">
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="text-xs uppercase tracking-wide text-zinc-500">
            {componentLabel(component.component)}
          </p>
          <h2 className="mt-2 text-lg font-semibold text-zinc-100">
            {component.summary}
          </h2>
        </div>
        <HealthIcon status={component.status} />
      </div>
      <StatusBadge status={component.status} />
      {component.details.length > 0 ? (
        <ul className="mt-4 space-y-2 text-sm text-zinc-400">
          {component.details.map((detail) => (
            <li key={detail} className="rounded-lg bg-zinc-950/60 px-3 py-2">
              {detail}
            </li>
          ))}
        </ul>
      ) : null}
    </article>
  );
}

function DiagnosticsShell({
  title,
  detail,
  action,
}: {
  title: string;
  detail?: string;
  action?: ReactNode;
}) {
  return (
    <section className="rounded-2xl border border-zinc-800 bg-zinc-900/70 p-6">
      <p className="text-sm uppercase tracking-wide text-zinc-500">
        Diagnostics
      </p>
      <h1 className="mt-3 text-2xl font-semibold text-zinc-50">{title}</h1>
      {detail ? <p className="mt-2 text-sm text-zinc-400">{detail}</p> : null}
      {action ? <div className="mt-4">{action}</div> : null}
    </section>
  );
}

function HealthIcon({ status }: { status: DiagnosticHealthStatus }) {
  const className = "h-5 w-5";
  switch (status) {
    case "healthy":
      return <CheckCircle2 className={`${className} text-emerald-400`} />;
    case "degraded":
      return <AlertTriangle className={`${className} text-amber-400`} />;
    case "unavailable":
      return <XCircle className={`${className} text-red-400`} />;
    case "unknown":
      return <CircleHelp className={`${className} text-zinc-400`} />;
  }
}

function StatusBadge({ status }: { status: DiagnosticHealthStatus }) {
  const className = {
    healthy: "border-emerald-500/40 bg-emerald-500/10 text-emerald-300",
    degraded: "border-amber-500/40 bg-amber-500/10 text-amber-300",
    unavailable: "border-red-500/40 bg-red-500/10 text-red-300",
    unknown: "border-zinc-500/40 bg-zinc-500/10 text-zinc-300",
  }[status];

  return (
    <span
      className={`mt-4 inline-flex w-fit rounded-full border px-3 py-1 text-xs font-medium uppercase tracking-wide ${className}`}
    >
      {status}
    </span>
  );
}

function componentLabel(component: DiagnosticComponentResponse["component"]) {
  switch (component) {
    case "database":
      return "Database";
    case "settings":
      return "Settings";
    case "sources":
      return "Sources";
    case "collector":
      return "Collector";
    case "runtime":
      return "Runtime";
  }
}

function logStatusText(status: DiagnosticsStatusResponse["logs"]["status"]) {
  switch (status) {
    case "available":
      return "Log folder can be opened from this device.";
    case "missing":
      return "No log folder exists yet. This is expected before logs are written.";
    case "unsupported":
      return "Opening logs is not supported on this platform.";
  }
}

function errorMessage(error: unknown) {
  if (error instanceof BurnlyClientError) {
    return error.message;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "Diagnostics could not be loaded.";
}

const secondaryButtonClass =
  "rounded-lg border border-zinc-700 px-3 py-2 text-sm font-medium text-zinc-200 transition hover:border-zinc-500 hover:bg-zinc-800";
