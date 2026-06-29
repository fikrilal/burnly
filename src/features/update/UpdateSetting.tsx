import { RefreshCw } from "lucide-react";
import {
  useUpdateState,
  useCheckForUpdate,
  useDownloadUpdate,
  useRestartForUpdate,
} from "./use-update";
import { Button } from "../../components/ui/button";
import { ErrorState } from "../../components/burnly";
import type { UpdateStatusResponse } from "../../ipc/generated/contracts";

interface UpdateActionProps {
  status: UpdateStatusResponse["status"];
  availableVersion: string | null;
  downloadedVersion: string | null;
  error: UpdateStatusResponse["error"];
  isMutating: boolean;
  isChecking: boolean;
  isDownloading: boolean;
  isRestarting: boolean;
  onCheck: () => void;
  onDownload: () => void;
  onRestart: () => void;
}

interface InteractiveUpdateRowProps {
  updateState: UpdateStatusResponse;
  isChecking: boolean;
  isDownloading: boolean;
  isRestarting: boolean;
  mutationError: unknown;
  onCheck: () => void;
  onDownload: () => void;
  onRestart: () => void;
}

interface UpdateViewProps {
  description: string;
  buttonLabel: string;
  isDisabled: boolean;
  showSpinner: boolean;
  onClick: () => void;
}

const UPDATER_ERROR_MESSAGES: Record<string, string> = {
  "update.unavailable": "Burnly updates are not available in this build.",
  "update.invalid_state": "Burnly cannot run that update operation right now.",
  "update.network_failed": "Burnly could not reach the update feed.",
  "update.signature_failed": "Burnly could not verify the update signature.",
  "update.install_failed": "Burnly could not install the downloaded update.",
  "update.internal": "Burnly could not complete the update operation.",
};

function getUpdateViewProps(props: UpdateActionProps): UpdateViewProps {
  return (
    getBusyUpdateViewProps(props) ??
    getActionUpdateViewProps(props) ?? {
      description: "Check for updates to get the latest features.",
      buttonLabel: "Check",
      isDisabled: props.isMutating,
      showSpinner: props.isChecking,
      onClick: props.onCheck,
    }
  );
}

function getBusyUpdateViewProps({
  status,
}: UpdateActionProps): UpdateViewProps | null {
  if (status === "checking") {
    return disabledUpdateAction("Checking for updates...", "Checking", true);
  }

  if (status === "downloading") {
    return disabledUpdateAction("Downloading update...", "Downloading", true);
  }

  return null;
}

function getActionUpdateViewProps(
  props: UpdateActionProps,
): UpdateViewProps | null {
  const {
    status,
    availableVersion,
    downloadedVersion,
    error,
    isMutating,
    isDownloading,
    isRestarting,
    onDownload,
    onRestart,
  } = props;

  if (status === "available") {
    return {
      description: `Version ${availableVersion ?? ""} is available.`,
      buttonLabel: "Install",
      isDisabled: isMutating,
      showSpinner: isDownloading,
      onClick: onDownload,
    };
  }

  if (status === "ready") {
    return {
      description: `Version ${downloadedVersion ?? availableVersion ?? ""} is ready.`,
      buttonLabel: "Restart",
      isDisabled: isMutating,
      showSpinner: isRestarting,
      onClick: onRestart,
    };
  }

  if (isTerminalFailure(status, error)) {
    return disabledUpdateAction(
      "Burnly cannot continue this update automatically.",
      "Check",
      false,
    );
  }

  return null;
}

function isTerminalFailure(
  status: UpdateStatusResponse["status"],
  error: UpdateStatusResponse["error"],
): boolean {
  return status === "failed" && error?.retryable === false;
}

function disabledUpdateAction(
  description: string,
  buttonLabel: string,
  showSpinner: boolean,
): UpdateViewProps {
  return {
    description,
    buttonLabel,
    isDisabled: true,
    showSpinner,
    onClick: () => {
      /* no-op */
    },
  };
}

interface UpdateErrorObject {
  code?: string;
  message?: string;
}

function isUpdateErrorObject(error: unknown): error is UpdateErrorObject {
  return typeof error === "object" && error !== null;
}

function getErrorMessage(error: unknown): string | null {
  if (!error) return null;
  if (error instanceof Error) return error.message;

  if (isUpdateErrorObject(error)) {
    if (typeof error.code === "string" && error.code) {
      const code = error.code;
      return UPDATER_ERROR_MESSAGES[code] ?? `Update failed (code: ${code})`;
    }
    if (typeof error.message === "string" && error.message) {
      return error.message;
    }
  }

  return "Failed to perform update operation.";
}

function DisabledUpdateRow({
  description,
  error,
}: {
  description: string;
  error?: unknown;
}) {
  const errorMessage = getErrorMessage(error);

  return (
    <div className="flex flex-col gap-2 py-3 opacity-50">
      <div className="flex items-center justify-between gap-4">
        <div className="flex flex-col gap-1">
          <span className="text-sm font-medium">Updates</span>
          <span className="text-xs text-muted-foreground leading-normal">
            {description}
          </span>
        </div>
        <Button variant="outline" size="xs" disabled>
          <span className="text-xs leading-none">Check</span>
        </Button>
      </div>
      {errorMessage ? (
        <div className="mt-1">
          <ErrorState title="Update failed" description={errorMessage} />
        </div>
      ) : null}
    </div>
  );
}

function LoadingUpdateRow() {
  return (
    <div className="flex items-center justify-between gap-4 py-3">
      <div className="flex flex-col gap-1">
        <span className="text-sm font-medium text-muted-foreground">
          Checking update status...
        </span>
      </div>
    </div>
  );
}

function InteractiveUpdateRow({
  updateState,
  isChecking,
  isDownloading,
  isRestarting,
  mutationError,
  onCheck,
  onDownload,
  onRestart,
}: InteractiveUpdateRowProps) {
  const { status, availableVersion, downloadedVersion, error } = updateState;
  const isMutating = isChecking || isDownloading || isRestarting;
  const viewProps = getUpdateViewProps({
    status,
    availableVersion,
    downloadedVersion,
    error,
    isMutating,
    isChecking,
    isDownloading,
    isRestarting,
    onCheck,
    onDownload,
    onRestart,
  });
  const errorMessage = getErrorMessage(error ?? mutationError);

  return (
    <div className="flex flex-col gap-2 py-3">
      <div className="flex items-center justify-between gap-4">
        <div className="flex flex-col gap-1">
          <span className="text-sm font-medium">Updates</span>
          <span className="text-xs text-muted-foreground leading-normal">
            {viewProps.description}
          </span>
        </div>
        <Button
          variant="outline"
          size="xs"
          disabled={viewProps.isDisabled}
          onClick={viewProps.onClick}
        >
          {viewProps.showSpinner ? (
            <RefreshCw className="mr-1 size-3 animate-spin" />
          ) : null}
          <span className="text-xs leading-none">{viewProps.buttonLabel}</span>
        </Button>
      </div>
      {errorMessage ? (
        <div className="mt-1">
          <ErrorState title="Update failed" description={errorMessage} />
        </div>
      ) : null}
    </div>
  );
}

export function UpdateSetting() {
  const { data: updateState, error: loadError, isError } = useUpdateState();
  const checkForUpdateMutation = useCheckForUpdate();
  const downloadUpdateMutation = useDownloadUpdate();
  const restartForUpdateMutation = useRestartForUpdate();

  if (!updateState && isError) {
    return (
      <DisabledUpdateRow
        description="Burnly could not load update status."
        error={loadError}
      />
    );
  }

  if (!updateState) return <LoadingUpdateRow />;

  if (updateState.status === "unavailable") {
    return (
      <DisabledUpdateRow description="Updates are not available for this build." />
    );
  }

  const isMutating =
    checkForUpdateMutation.isPending ||
    downloadUpdateMutation.isPending ||
    restartForUpdateMutation.isPending;

  const handleCheck = () => {
    if (!isMutating) {
      checkForUpdateMutation.mutate();
    }
  };

  const handleDownload = () => {
    if (!isMutating) {
      downloadUpdateMutation.mutate();
    }
  };

  const handleRestart = () => {
    if (!isMutating) {
      restartForUpdateMutation.mutate();
    }
  };

  const mutationError =
    checkForUpdateMutation.error ??
    downloadUpdateMutation.error ??
    restartForUpdateMutation.error;

  return (
    <InteractiveUpdateRow
      updateState={updateState}
      isChecking={checkForUpdateMutation.isPending}
      isDownloading={downloadUpdateMutation.isPending}
      isRestarting={restartForUpdateMutation.isPending}
      mutationError={mutationError}
      onCheck={handleCheck}
      onDownload={handleDownload}
      onRestart={handleRestart}
    />
  );
}
