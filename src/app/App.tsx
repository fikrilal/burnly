import { useEffect, useState } from "react";
import { Activity, Database, ShieldCheck } from "lucide-react";

import {
  getAppBootstrap,
  getAppCapabilities,
  type CommandResult,
} from "../ipc/client";
import { BurnlyClientError } from "../ipc/errors";
import type {
  AppBootstrapResponse,
  AppCapabilitiesResponse,
} from "../ipc/generated/contracts";
import { CONTRACT_VERSION, EVENT_NAMES } from "../ipc/generated/contracts";
import { subscribeToEvent } from "../ipc/events";

type LoadBootstrap = () => Promise<CommandResult<AppBootstrapResponse>>;
type LoadCapabilities = () => Promise<CommandResult<AppCapabilitiesResponse>>;

interface AppProps {
  loadBootstrap?: LoadBootstrap;
  loadCapabilities?: LoadCapabilities;
}

type AppState =
  | {
      status: "loading";
    }
  | {
      status: "ready";
      bootstrap: AppBootstrapResponse;
      capabilities: AppCapabilitiesResponse;
    }
  | {
      status: "failed";
      title: string;
      message: string;
    }
  | {
      status: "incompatible";
      runtimeContractVersion: number;
      frontendContractVersion: number;
    };

import { Overview } from "../features/overview";
import { BudgetsView } from "../features/budgets/BudgetsView";
import { CalendarView } from "../features/calendar/CalendarView";
import { SessionsView } from "../features/sessions/SessionsView";
import { SettingsView } from "../features/settings/SettingsView";

type ViewMode = "overview" | "calendar" | "sessions" | "budgets" | "settings";

export function App({
  loadBootstrap = getAppBootstrap,
  loadCapabilities = getAppCapabilities,
}: AppProps) {
  const state = useStartupState(loadBootstrap, loadCapabilities);

  const [viewMode, setViewMode] = useState<ViewMode>("overview");

  return (
    <main className="min-h-screen bg-zinc-950 text-zinc-50">
      <div className="mx-auto w-full max-w-6xl px-6 py-10">
        {state.status === "ready" ? (
          <div className="flex flex-col gap-6">
            <div className="flex gap-2 border-b border-zinc-800 pb-2">
              <button
                type="button"
                onClick={() => {
                  setViewMode("overview");
                }}
                className={`px-4 py-2 text-sm font-medium transition-colors ${
                  viewMode === "overview"
                    ? "border-b-2 border-cyan-400 text-cyan-400"
                    : "text-zinc-400 hover:text-zinc-200"
                }`}
              >
                Overview
              </button>
              <button
                type="button"
                onClick={() => {
                  setViewMode("calendar");
                }}
                className={`px-4 py-2 text-sm font-medium transition-colors ${
                  viewMode === "calendar"
                    ? "border-b-2 border-cyan-400 text-cyan-400"
                    : "text-zinc-400 hover:text-zinc-200"
                }`}
              >
                Calendar
              </button>
              <button
                type="button"
                onClick={() => {
                  setViewMode("sessions");
                }}
                className={`px-4 py-2 text-sm font-medium transition-colors ${
                  viewMode === "sessions"
                    ? "border-b-2 border-cyan-400 text-cyan-400"
                    : "text-zinc-400 hover:text-zinc-200"
                }`}
              >
                Sessions
              </button>
              <button
                type="button"
                onClick={() => {
                  setViewMode("settings");
                }}
                className={`px-4 py-2 text-sm font-medium transition-colors ${
                  viewMode === "settings"
                    ? "border-b-2 border-cyan-400 text-cyan-400"
                    : "text-zinc-400 hover:text-zinc-200"
                }`}
              >
                Settings
              </button>
              {state.bootstrap.features.budgets ? (
                <button
                  type="button"
                  onClick={() => {
                    setViewMode("budgets");
                  }}
                  className={`px-4 py-2 text-sm font-medium transition-colors ${
                    viewMode === "budgets"
                      ? "border-b-2 border-cyan-400 text-cyan-400"
                      : "text-zinc-400 hover:text-zinc-200"
                  }`}
                >
                  Budgets
                </button>
              ) : null}
            </div>
            {viewMode === "overview" && (
              <Overview
                reportingTimezone={state.bootstrap.settings.reportingTimezone}
              />
            )}
            {viewMode === "calendar" && (
              <CalendarView
                reportingTimezone={state.bootstrap.settings.reportingTimezone}
              />
            )}
            {viewMode === "sessions" && <SessionsView />}
            {viewMode === "budgets" && <BudgetsView />}
            {viewMode === "settings" && (
              <SettingsView capabilities={state.capabilities} />
            )}
          </div>
        ) : (
          <div className="mt-12 grid gap-4 md:grid-cols-3">
            {renderCards(state)}
          </div>
        )}
      </div>
    </main>
  );
}

function useStartupState(
  loadBootstrap: LoadBootstrap,
  loadCapabilities: LoadCapabilities,
): AppState {
  const [state, setState] = useState<AppState>({ status: "loading" });

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    async function load() {
      try {
        const bootstrap = await loadBootstrap();

        if (!isCompatibleContractVersion(bootstrap.data.contractVersion)) {
          if (active) {
            setState({
              status: "incompatible",
              runtimeContractVersion: bootstrap.data.contractVersion,
              frontendContractVersion: CONTRACT_VERSION,
            });
          }
          return;
        }

        const capabilities = await loadCapabilities();

        if (active) {
          setState({
            status: "ready",
            bootstrap: bootstrap.data,
            capabilities: capabilities.data,
          });
        }
      } catch (error) {
        if (active) {
          setState({
            status: "failed",
            ...failureContent(error),
          });
        }
      }
    }

    const setupListener = async () => {
      const fn = await subscribeToEvent(EVENT_NAMES.settingsChanged, () => {
        void load();
      });
      if (active) {
        unlisten = fn;
      } else {
        fn();
      }
    };

    void load();
    void setupListener();

    return () => {
      active = false;
      if (unlisten) {
        unlisten();
      }
    };
  }, [loadBootstrap, loadCapabilities]);

  return state;
}

function renderCards(state: AppState) {
  if (state.status === "loading") {
    return (
      <StatusCard
        icon={Activity}
        label="Runtime"
        value="Loading"
        detail="Reading local state"
      />
    );
  }

  if (state.status === "failed") {
    return (
      <StatusCard
        icon={Activity}
        label="Runtime"
        value={state.title}
        detail={state.message}
      />
    );
  }

  if (state.status === "incompatible") {
    return (
      <StatusCard
        icon={ShieldCheck}
        label="Contract"
        value="Incompatible"
        detail={`Frontend v${state.frontendContractVersion}, runtime v${state.runtimeContractVersion}`}
      />
    );
  }

  return (
    <>
      <StatusCard
        icon={ShieldCheck}
        label="App"
        value={`v${state.bootstrap.appVersion}`}
        detail={`Contract v${state.bootstrap.contractVersion}`}
      />
      <StatusCard
        icon={Database}
        label="Storage"
        value={`Schema ${state.bootstrap.database.schemaVersion}`}
        detail={state.bootstrap.settings.reportingTimezone}
      />
      <StatusCard
        icon={Activity}
        label="Collectors"
        value={sourceValue(state.bootstrap)}
        detail={capabilityDetail(state.capabilities)}
      />
    </>
  );
}

function sourceValue(bootstrap: AppBootstrapResponse): string {
  return `${bootstrap.sources.enabledCount} enabled`;
}

function capabilityDetail(capabilities: AppCapabilitiesResponse): string {
  return capabilities.tray.status.replace("_", " ");
}

function isCompatibleContractVersion(runtimeContractVersion: number): boolean {
  return runtimeContractVersion === CONTRACT_VERSION;
}

function failureContent(error: unknown): { title: string; message: string } {
  if (error instanceof BurnlyClientError) {
    return {
      title:
        error.kind === "application"
          ? "Application error"
          : "Runtime unavailable",
      message: error.message,
    };
  }

  return {
    title: "Runtime unavailable",
    message:
      error instanceof Error
        ? error.message
        : "Burnly could not load local runtime state.",
  };
}

interface StatusCardProps {
  icon: typeof Activity;
  label: string;
  value: string;
  detail: string;
}

function StatusCard({ icon: Icon, label, value, detail }: StatusCardProps) {
  return (
    <div className="rounded-lg border border-zinc-800 bg-zinc-900/70 p-5">
      <Icon className="h-5 w-5 text-cyan-300" aria-hidden />
      <p className="mt-5 text-sm text-zinc-400">{label}</p>
      <p className="mt-1 text-xl font-semibold text-white">{value}</p>
      <p className="mt-2 text-sm text-zinc-500">{detail}</p>
    </div>
  );
}
