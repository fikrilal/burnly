import { useEffect, useState } from "react";

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
      recovery: boolean;
    }
  | {
      status: "incompatible";
      runtimeContractVersion: number;
      frontendContractVersion: number;
    };

import { StyleguideView } from "../features/styleguide/StyleguideView";
import { TrayPanel, TrayStartupState } from "../features/tray";

export function App({
  loadBootstrap = getAppBootstrap,
  loadCapabilities = getAppCapabilities,
}: AppProps) {
  const state = useStartupState(loadBootstrap, loadCapabilities);
  const surface = appSurface();

  if (surface === "styleguide") {
    return <StyleguideView />;
  }

  return <TraySurface state={state} />;
}

function TraySurface({ state }: { state: AppState }) {
  if (state.status === "ready") {
    const reportingTimezone = Intl.DateTimeFormat().resolvedOptions().timeZone;
    return <TrayPanel reportingTimezone={reportingTimezone} />;
  }

  if (state.status === "incompatible") {
    return (
      <TrayStartupState
        status="Contract Incompatible"
        detail={`Frontend v${state.frontendContractVersion}, runtime v${state.runtimeContractVersion}`}
      />
    );
  }

  if (state.status === "failed") {
    return <TrayStartupState status={state.title} detail={state.message} />;
  }

  return <TrayStartupState status="Loading" detail="Starting Burnly runtime" />;
}

function appSurface(): "tray" | "styleguide" {
  if (window.location.hash === "#/styleguide") {
    return "styleguide";
  }
  return "tray";
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

function isCompatibleContractVersion(runtimeContractVersion: number): boolean {
  return runtimeContractVersion === CONTRACT_VERSION;
}

function failureContent(error: unknown): {
  title: string;
  message: string;
  recovery: boolean;
} {
  if (error instanceof BurnlyClientError) {
    return {
      title:
        error.code === "bootstrap.recovery_required"
          ? "Database recovery required"
          : error.kind === "application"
            ? "Application error"
            : "Runtime unavailable",
      message: error.message,
      recovery: error.code === "bootstrap.recovery_required",
    };
  }

  return {
    title: "Runtime unavailable",
    message:
      error instanceof Error
        ? error.message
        : "Burnly could not load local runtime state.",
    recovery: false,
  };
}
