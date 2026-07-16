import type { ReactNode } from "react";

import type {
  AccountSessionResponse,
  AppCapabilitiesResponse,
  SettingsResponse,
} from "../../ipc/generated/contracts";
import { ErrorState } from "../../components/burnly";
import { Button } from "../../components/ui/button";
import { Switch } from "../../components/ui/switch";
import { ThemeToggle } from "../../components/ui/theme-toggle";
import { userSafeErrorMessage } from "../../lib/user-safe-error";
import {
  DiagnosticsEntrySetting,
  DiagnosticsPage,
} from "../diagnostics/DiagnosticsPage";
import { UpdateSetting } from "../update/UpdateSetting";
import {
  accountErrorMessage,
  accountSessionErrorMessage,
} from "./account-errors";
import {
  useAccountSession,
  useCancelAccountLogin,
  useLogoutAccount,
  useStartAccountLogin,
} from "./use-account";
import { useSettings, useUpdateSettings } from "./use-settings";

export function SettingsTab({
  page,
  appVersion,
  launchAtLoginCapability,
  diagnosticsCapabilities,
  onPageChange,
}: {
  page: "list" | "diagnostics";
  appVersion: string;
  launchAtLoginCapability: AppCapabilitiesResponse["launchAtLogin"];
  diagnosticsCapabilities: AppCapabilitiesResponse["diagnostics"];
  onPageChange: (page: "list" | "diagnostics") => void;
}) {
  const settings = useSettings();
  const updateSettings = useUpdateSettings();

  if (settings.isPending) {
    return <SettingsLoading />;
  }

  if (settings.isError) {
    return (
      <SettingsLoadError
        error={settings.error}
        onRetry={() => {
          void settings.refetch();
        }}
      />
    );
  }

  return (
    <SettingsForm
      settings={settings.data}
      page={page}
      appVersion={appVersion}
      launchAtLoginCapability={launchAtLoginCapability}
      diagnosticsCapabilities={diagnosticsCapabilities}
      isSaving={updateSettings.isPending}
      saveError={updateSettings.error}
      onPageChange={onPageChange}
      onUpdate={(request) => {
        updateSettings.mutate(request);
      }}
    />
  );
}

function SettingsForm({
  settings,
  page,
  appVersion,
  launchAtLoginCapability,
  diagnosticsCapabilities,
  isSaving,
  saveError,
  onPageChange,
  onUpdate,
}: {
  settings: SettingsResponse;
  page: "list" | "diagnostics";
  appVersion: string;
  launchAtLoginCapability: AppCapabilitiesResponse["launchAtLogin"];
  diagnosticsCapabilities: AppCapabilitiesResponse["diagnostics"];
  isSaving: boolean;
  saveError: Error | null;
  onPageChange: (page: "list" | "diagnostics") => void;
  onUpdate: (request: {
    launchAtLogin: boolean;
    closeBehavior: SettingsResponse["closeBehavior"];
    expectedRevision: number;
  }) => void;
}) {
  const { changeCloseBehavior, changeLaunchAtLogin } = useSettingsFormActions({
    settings,
    launchAtLoginCapability,
    onUpdate,
  });

  if (page === "diagnostics") {
    return (
      <SettingsPageShell appVersion={appVersion}>
        <DiagnosticsPage
          capabilities={diagnosticsCapabilities}
          onBack={() => {
            onPageChange("list");
          }}
        />
      </SettingsPageShell>
    );
  }

  return (
    <SettingsPageShell appVersion={appVersion}>
      <div className="flex flex-col">
        <SettingsList
          settings={settings}
          launchAtLoginCapability={launchAtLoginCapability}
          isSaving={isSaving}
          onChangeLaunchAtLogin={changeLaunchAtLogin}
          onChangeCloseBehavior={changeCloseBehavior}
          onOpenDiagnostics={() => {
            onPageChange("diagnostics");
          }}
        />
        <SettingsSaveError error={saveError} />
      </div>
    </SettingsPageShell>
  );
}

function useSettingsFormActions({
  settings,
  launchAtLoginCapability,
  onUpdate,
}: {
  settings: SettingsResponse;
  launchAtLoginCapability: AppCapabilitiesResponse["launchAtLogin"];
  onUpdate: (request: {
    launchAtLogin: boolean;
    closeBehavior: SettingsResponse["closeBehavior"];
    expectedRevision: number;
  }) => void;
}) {
  const changeCloseBehavior = (
    closeBehavior: SettingsResponse["closeBehavior"],
  ) => {
    if (closeBehavior === settings.closeBehavior) return;
    onUpdate({
      launchAtLogin: settings.launchAtLogin,
      closeBehavior,
      expectedRevision: settings.revision,
    });
  };

  const changeLaunchAtLogin = (launchAtLogin: boolean) => {
    if (!launchAtLoginCapability.supported) return;
    if (launchAtLogin === settings.launchAtLogin) return;
    onUpdate({
      launchAtLogin,
      closeBehavior: settings.closeBehavior,
      expectedRevision: settings.revision,
    });
  };

  return { changeCloseBehavior, changeLaunchAtLogin };
}

function SettingsPageShell({
  appVersion,
  children,
}: {
  appVersion: string;
  children: ReactNode;
}) {
  return (
    <div className="flex min-h-full flex-1 flex-col justify-between gap-4">
      {children}
      <SettingsVersion appVersion={appVersion} />
    </div>
  );
}

function SettingsList({
  settings,
  launchAtLoginCapability,
  isSaving,
  onChangeLaunchAtLogin,
  onChangeCloseBehavior,
  onOpenDiagnostics,
}: {
  settings: SettingsResponse;
  launchAtLoginCapability: AppCapabilitiesResponse["launchAtLogin"];
  isSaving: boolean;
  onChangeLaunchAtLogin: (value: boolean) => void;
  onChangeCloseBehavior: (value: SettingsResponse["closeBehavior"]) => void;
  onOpenDiagnostics: () => void;
}) {
  return (
    <div className="flex flex-col divide-y divide-border">
      <AccountSetting />
      <LaunchAtLoginSetting
        value={settings.launchAtLogin}
        isDisabled={isSaving || !launchAtLoginCapability.supported}
        onChange={onChangeLaunchAtLogin}
      />
      <CloseBehaviorSetting
        value={settings.closeBehavior}
        isSaving={isSaving}
        onChange={onChangeCloseBehavior}
      />
      <ThemeSetting />
      <UpdateSetting />
      <DiagnosticsEntrySetting onOpen={onOpenDiagnostics} />
    </div>
  );
}

function AccountSetting() {
  const account = useAccountSession();

  if (account.isPending) {
    return <AccountSettingLoading />;
  }

  if (account.isError) {
    return (
      <AccountSettingError
        error={account.error}
        onRetry={() => {
          void account.refetch();
        }}
      />
    );
  }

  // After pending/error guards, React Query types `data` as defined.
  return <AccountSettingActive session={account.data} />;
}

function AccountSettingLoading() {
  return (
    <div className="flex items-center justify-between gap-4 py-3">
      <div className="flex flex-col gap-1">
        <span className="text-sm font-medium">Account</span>
        <span className="text-xs text-muted-foreground leading-normal">
          Loading account status
        </span>
      </div>
    </div>
  );
}

function AccountSettingError({
  error,
  onRetry,
}: {
  error: unknown;
  onRetry: () => void;
}) {
  return (
    <div className="flex items-center justify-between gap-4 py-3">
      <div className="flex flex-col gap-1">
        <span className="text-sm font-medium">Account</span>
        <span className="text-xs text-muted-foreground leading-normal">
          {accountErrorMessage(error, "Account status is unavailable.")}
        </span>
      </div>
      <AccountActionButton disabled={false} onClick={onRetry} label="Retry" />
    </div>
  );
}

function accountDetail(
  status: AccountSessionResponse["status"],
  email: string | null,
): string {
  if (status === "signed_in") return email ?? "Signed in";
  if (status === "waiting_for_browser") {
    return "Complete sign-in in your browser…";
  }
  if (status === "exchanging") return "Signing in…";
  return "Not signed in";
}

function accountErrorText(
  session: AccountSessionResponse,
  mutationError: Error | null,
): string | null {
  if (mutationError) return accountErrorMessage(mutationError);
  if (session.lastErrorCode !== null || session.lastErrorMessage !== null) {
    return accountSessionErrorMessage(
      session.lastErrorCode,
      session.lastErrorMessage,
    );
  }
  return null;
}

type AccountMutation = ReturnType<typeof useStartAccountLogin>;

function deriveAccountSettingViewState(
  session: AccountSessionResponse,
  startLogin: AccountMutation,
  cancelLogin: AccountMutation,
  logout: AccountMutation,
) {
  const status = session.status;
  const actionPending =
    startLogin.isPending || cancelLogin.isPending || logout.isPending;
  const mutationError =
    startLogin.error ?? cancelLogin.error ?? logout.error ?? null;
  const errorText = accountErrorText(session, mutationError);
  const detail = accountDetail(status, session.email);
  const showRetry =
    status === "signed_out" && Boolean(errorText) && !actionPending;
  return { status, actionPending, errorText, detail, showRetry };
}

function AccountSettingActive({
  session,
}: {
  session: AccountSessionResponse;
}) {
  const startLogin = useStartAccountLogin();
  const cancelLogin = useCancelAccountLogin();
  const logout = useLogoutAccount();

  const { status, actionPending, errorText, detail, showRetry } =
    deriveAccountSettingViewState(session, startLogin, cancelLogin, logout);

  return (
    <div className="flex items-center justify-between gap-4 py-3">
      <div className="flex min-w-0 flex-col gap-1">
        <span className="text-sm font-medium">Account</span>
        <span className="truncate text-xs text-muted-foreground leading-normal">
          {detail}
        </span>
        {errorText ? (
          <span className="text-xs text-destructive leading-normal">
            {errorText}
          </span>
        ) : null}
      </div>
      <AccountSettingActions
        status={status}
        showRetry={showRetry}
        actionPending={actionPending}
        startLogin={startLogin}
        cancelLogin={cancelLogin}
        logout={logout}
      />
    </div>
  );
}

function AccountSettingActions({
  status,
  showRetry,
  actionPending,
  startLogin,
  cancelLogin,
  logout,
}: {
  status: AccountSessionResponse["status"];
  showRetry: boolean;
  actionPending: boolean;
  startLogin: AccountMutation;
  cancelLogin: AccountMutation;
  logout: AccountMutation;
}) {
  return (
    <div className="flex shrink-0 items-center gap-2">
      {status === "signed_out" ? (
        <AccountActionButton
          disabled={actionPending}
          onClick={() => {
            startLogin.reset();
            startLogin.mutate();
          }}
          label={
            startLogin.isPending
              ? "Opening…"
              : showRetry
                ? "Try again"
                : "Sign in"
          }
        />
      ) : null}
      {status === "waiting_for_browser" ? (
        <AccountActionButton
          disabled={actionPending}
          onClick={() => {
            cancelLogin.mutate();
          }}
          label={cancelLogin.isPending ? "Cancelling…" : "Cancel"}
        />
      ) : null}
      {status === "exchanging" ? (
        <span className="text-xs text-muted-foreground">Please wait…</span>
      ) : null}
      {status === "signed_in" ? (
        <AccountActionButton
          disabled={actionPending}
          onClick={() => {
            logout.mutate();
          }}
          label={logout.isPending ? "Signing out…" : "Sign out"}
        />
      ) : null}
    </div>
  );
}

function AccountActionButton({
  label,
  disabled,
  onClick,
}: {
  label: string;
  disabled: boolean;
  onClick: () => void;
}) {
  return (
    <Button variant="outline" size="xs" disabled={disabled} onClick={onClick}>
      <span className="text-xs leading-none">{label}</span>
    </Button>
  );
}

function SettingsVersion({ appVersion }: { appVersion: string }) {
  return (
    <div className="text-center text-[10px] font-mono tracking-widest text-muted-foreground/40 uppercase">
      Version {appVersion}
    </div>
  );
}

function SettingsSaveError({ error }: { error: Error | null }) {
  if (!error) return null;

  return (
    <div className="mt-4">
      <ErrorState
        title="Settings not saved"
        description={userSafeErrorMessage(
          error,
          "Burnly could not save settings.",
        )}
      />
    </div>
  );
}

function ThemeSetting() {
  return (
    <div className="flex items-center justify-between gap-4 py-3">
      <div className="flex flex-col gap-1">
        <span className="text-sm font-medium">Theme</span>
        <span className="text-xs text-muted-foreground leading-normal">
          Select the interface color mode.
        </span>
      </div>
      <ThemeToggle />
    </div>
  );
}

function SettingsLoading() {
  return (
    <div className="flex flex-col gap-3">
      <p className="text-sm text-muted-foreground">Loading settings</p>
    </div>
  );
}

function SettingsLoadError({
  error,
  onRetry,
}: {
  error: unknown;
  onRetry: () => void;
}) {
  return (
    <div className="flex flex-col gap-3">
      <ErrorState
        title="Settings unavailable"
        description={userSafeErrorMessage(
          error,
          "Burnly could not load settings.",
        )}
      />
      <Button variant="outline" size="sm" onClick={onRetry} className="w-fit">
        Retry
      </Button>
    </div>
  );
}

function LaunchAtLoginSetting({
  value,
  isDisabled,
  onChange,
}: {
  value: boolean;
  isDisabled: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between gap-4 py-3">
      <div className="flex flex-col gap-1">
        <span className="text-sm font-medium">Launch at login</span>
        <span className="text-xs text-muted-foreground leading-normal">
          Start Burnly automatically when you log into your system.
        </span>
      </div>
      <Switch
        checked={value}
        disabled={isDisabled}
        aria-label="Launch at login"
        onCheckedChange={onChange}
      />
    </div>
  );
}

function CloseBehaviorSetting({
  value,
  isSaving,
  onChange,
}: {
  value: SettingsResponse["closeBehavior"];
  isSaving: boolean;
  onChange: (value: SettingsResponse["closeBehavior"]) => void;
}) {
  const isQuit = value === "quit";

  return (
    <div className="flex items-center justify-between gap-4 py-3">
      <div className="flex flex-col gap-1">
        <span className="text-sm font-medium">Quit on close</span>
        <span className="text-xs text-muted-foreground leading-normal">
          Terminate the application when closing the panel.
        </span>
      </div>
      <div className="flex items-center gap-3">
        <Switch
          checked={isQuit}
          disabled={isSaving}
          aria-label="Quit on close"
          onCheckedChange={(checked) => {
            onChange(checked ? "quit" : "hide");
          }}
        />
      </div>
    </div>
  );
}
