import type { ReactNode } from "react";

import type {
  AppCapabilitiesResponse,
  SettingsResponse,
} from "../../ipc/generated/contracts";
import { ErrorState } from "../../components/burnly";
import { Switch } from "../../components/ui/switch";
import { ThemeToggle } from "../../components/ui/theme-toggle";
import { userSafeErrorMessage } from "../../lib/user-safe-error";
import {
  DiagnosticsEntrySetting,
  DiagnosticsPage,
} from "../diagnostics/DiagnosticsPage";
import { UpdateSetting } from "../update/UpdateSetting";
import { useAccountSession, useLogoutAccount } from "./use-account";
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
  const logout = useLogoutAccount();

  if (account.isPending) {
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

  if (account.isError) {
    return (
      <div className="flex items-center justify-between gap-4 py-3">
        <div className="flex flex-col gap-1">
          <span className="text-sm font-medium">Account</span>
          <span className="text-xs text-muted-foreground leading-normal">
            {userSafeErrorMessage(
              account.error,
              "Account status is unavailable.",
            )}
          </span>
        </div>
      </div>
    );
  }

  const session = account.data;
  const signedIn = session?.status === "signed_in";

  return (
    <div className="flex items-center justify-between gap-4 py-3">
      <div className="flex min-w-0 flex-col gap-1">
        <span className="text-sm font-medium">Account</span>
        <span className="truncate text-xs text-muted-foreground leading-normal">
          {signedIn
            ? (session.email ?? "Signed in")
            : "Not signed in"}
        </span>
        {logout.isError ? (
          <span className="text-xs text-destructive leading-normal">
            {userSafeErrorMessage(
              logout.error,
              "Burnly could not sign out.",
            )}
          </span>
        ) : null}
      </div>
      {signedIn ? (
        <button
          type="button"
          disabled={logout.isPending}
          onClick={() => {
            logout.mutate();
          }}
          className="shrink-0 rounded-md border border-border px-2.5 py-1 text-xs font-medium transition-colors hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none disabled:opacity-50"
        >
          {logout.isPending ? "Signing out…" : "Sign out"}
        </button>
      ) : null}
    </div>
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
      <button
        type="button"
        onClick={onRetry}
        className="w-fit rounded-md border border-border px-3 py-1.5 text-sm font-medium transition-colors hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
      >
        Retry
      </button>
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
