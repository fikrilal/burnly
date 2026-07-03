import { AlertTriangle, ChevronLeft, ChevronRight } from "lucide-react";

import type {
  AppCapabilitiesResponse,
  DiagnosticsHealthResponse,
} from "../../ipc/generated/contracts";
import { ErrorState } from "../../components/burnly";
import { Button } from "../../components/ui/button";
import { openExternalLink } from "../../ipc/external-links";
import { cn } from "../../lib/cn";
import { userSafeErrorMessage } from "../../lib/user-safe-error";
import {
  useCopyDiagnosticsReport,
  useDiagnosticsHealth,
  useExportDiagnosticsReport,
} from "./use-diagnostics";

const DIAGNOSTICS_ISSUES_URL = "https://github.com/fikrilal/burnly/issues";

export function DiagnosticsEntrySetting({ onOpen }: { onOpen: () => void }) {
  const health = useDiagnosticsHealth();
  const showWarning =
    health.data?.status === "warning" || health.data?.status === "error";

  return (
    <button
      type="button"
      aria-label="Open diagnostics"
      className="flex items-center justify-between gap-4 py-3 text-left transition-colors hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
      onClick={onOpen}
    >
      <div className="flex flex-col gap-1">
        <span className="text-sm font-medium">Diagnostics</span>
        <span className="text-xs text-muted-foreground leading-normal">
          View app health and export a local report.
        </span>
      </div>
      <span className="flex shrink-0 items-center gap-2">
        {showWarning ? (
          <AlertTriangle
            className="size-4 text-amber-300"
            aria-label="Diagnostics problem detected"
          />
        ) : null}
        <ChevronRight className="size-4 text-muted-foreground" />
      </span>
    </button>
  );
}

export function DiagnosticsPage({
  capabilities,
  onBack,
}: {
  capabilities: AppCapabilitiesResponse["diagnostics"];
  onBack: () => void;
}) {
  const health = useDiagnosticsHealth();
  const exportReport = useExportDiagnosticsReport();
  const copyReport = useCopyDiagnosticsReport();
  const supportNote = getDiagnosticsSupportNote(health.data);
  const mutationError = exportReport.error ?? copyReport.error;
  const isMutating = exportReport.isPending || copyReport.isPending;

  return (
    <div className="flex flex-col gap-4 py-1">
      <button
        type="button"
        className="-ml-0.5 inline-flex h-5 w-fit items-center gap-0.5 rounded-md pr-1 text-muted-foreground transition-colors hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
        onClick={onBack}
      >
        <ChevronLeft className="size-3" />
        <span className="text-xs leading-4 font-medium">Settings</span>
      </button>
      <div className="flex flex-col gap-1">
        <span className="text-base font-semibold">Diagnostics</span>
        <span className="text-xs text-muted-foreground leading-normal">
          Export or copy a local report to help debug app issues.
        </span>
      </div>
      <DiagnosticsActions
        isBusy={isMutating || health.isPending}
        canSend={capabilities.sendReport.supported}
        onExport={() => {
          copyReport.reset();
          exportReport.mutate();
        }}
        onCopy={() => {
          exportReport.reset();
          copyReport.mutate();
        }}
      />
      <DiagnosticsActionStatus
        exportStatus={exportReport.data?.status}
        copyStatus={copyReport.data?.status}
      />
      {mutationError ? <DiagnosticsActionError error={mutationError} /> : null}
      <DiagnosticsSupportNote note={supportNote} />
    </div>
  );
}

function DiagnosticsSupportNote({
  note,
}: {
  note: { message: string; className: string; showIssueLink: boolean };
}) {
  return (
    <p className={cn("mt-2 text-xs leading-normal", note.className)}>
      {note.message}
      {note.showIssueLink ? (
        <>
          {" "}
          Attach the report in a{" "}
          <a
            href={DIAGNOSTICS_ISSUES_URL}
            className="font-medium text-foreground underline underline-offset-2 transition-colors hover:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
            onClick={(event) => {
              event.preventDefault();
              void openExternalLink(DIAGNOSTICS_ISSUES_URL);
            }}
          >
            GitHub issue
          </a>
          .
        </>
      ) : null}
    </p>
  );
}

function DiagnosticsActions({
  isBusy,
  canSend,
  onExport,
  onCopy,
}: {
  isBusy: boolean;
  canSend: boolean;
  onExport: () => void;
  onCopy: () => void;
}) {
  return (
    <div className="flex shrink-0 items-center gap-2">
      <Button
        type="button"
        variant="outline"
        size="xs"
        disabled={isBusy}
        onClick={onExport}
      >
        <span className="text-xs leading-none">Export</span>
      </Button>
      <Button
        type="button"
        variant="outline"
        size="xs"
        disabled={isBusy}
        onClick={onCopy}
      >
        <span className="text-xs leading-none">Copy</span>
      </Button>
      <Button
        type="button"
        variant="outline"
        size="xs"
        disabled
        title={
          canSend
            ? "Report sending needs a backend endpoint."
            : "Report sending is coming later."
        }
      >
        <span className="text-xs leading-none">Send</span>
      </Button>
    </div>
  );
}

function DiagnosticsActionStatus({
  exportStatus,
  copyStatus,
}: {
  exportStatus: "exported" | "cancelled" | undefined;
  copyStatus: "copied" | undefined;
}) {
  const statusText =
    exportStatus === "exported"
      ? "Diagnostics report exported."
      : copyStatus === "copied"
        ? "Diagnostics report copied."
        : null;

  if (!statusText) return null;

  return <span className="text-xs text-muted-foreground">{statusText}</span>;
}

function DiagnosticsActionError({ error }: { error: unknown }) {
  return (
    <ErrorState
      title="Diagnostics failed"
      description={userSafeErrorMessage(
        error,
        "Burnly could not create the diagnostics report.",
      )}
    />
  );
}

function getDiagnosticsSupportNote(
  health: DiagnosticsHealthResponse | undefined,
): {
  message: string;
  className: string;
  showIssueLink: boolean;
} {
  if (!health) {
    return {
      message: "Checking diagnostics status...",
      className: "text-muted-foreground",
      showIssueLink: false,
    };
  }

  if (health.status === "error") {
    return {
      message:
        "Burnly detected an error. Export or copy diagnostics before reporting it.",
      className: "text-amber-300",
      showIssueLink: true,
    };
  }

  if (health.status === "warning") {
    return {
      message:
        "Burnly detected a problem. Export or copy diagnostics before reporting it.",
      className: "text-amber-300",
      showIssueLink: true,
    };
  }

  return {
    message: "No problems detected. Export diagnostics only if support asks.",
    className: "text-muted-foreground",
    showIssueLink: false,
  };
}
