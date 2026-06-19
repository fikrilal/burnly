import type { ReactNode } from "react";
import { AlertTriangle, CheckCircle2, CircleHelp, XCircle } from "lucide-react";

import { BurnlyClientError } from "../../ipc/errors";
import type {
  DiagnosticComponentResponse,
  DiagnosticHealthStatus,
  DiagnosticsStatusResponse,
  RevealLogsResponse,
} from "../../ipc/generated/contracts";
import { useDiagnostics, useRevealDiagnosticsLogs } from "./use-diagnostics";

export function DiagnosticsView() {
  const query = useDiagnostics();
  const revealLogs = useRevealDiagnosticsLogs();

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
    </section>
  );
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
