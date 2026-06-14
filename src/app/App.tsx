import { useEffect, useState } from "react";
import { Activity, Database, ShieldCheck } from "lucide-react";

import {
  getAppBootstrap,
  getAppCapabilities,
  type CommandResult,
} from "../ipc/client";
import type {
  AppBootstrapResponse,
  AppCapabilitiesResponse,
} from "../ipc/generated/contracts";

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
      message: string;
    };

export function App({
  loadBootstrap = getAppBootstrap,
  loadCapabilities = getAppCapabilities,
}: AppProps) {
  const [state, setState] = useState<AppState>({ status: "loading" });

  useEffect(() => {
    let active = true;

    async function load() {
      try {
        const [bootstrap, capabilities] = await Promise.all([
          loadBootstrap(),
          loadCapabilities(),
        ]);

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
            message:
              error instanceof Error
                ? error.message
                : "Burnly could not load local runtime state.",
          });
        }
      }
    }

    void load();

    return () => {
      active = false;
    };
  }, [loadBootstrap, loadCapabilities]);

  return (
    <main className="min-h-screen bg-zinc-950 text-zinc-50">
      <section className="mx-auto flex min-h-screen w-full max-w-6xl flex-col justify-center px-6 py-10">
        <div className="max-w-3xl">
          <p className="text-sm font-medium uppercase tracking-[0.18em] text-cyan-300">
            Local AI usage tracker
          </p>
          <h1 className="mt-4 text-5xl font-semibold tracking-normal text-white">
            Burnly
          </h1>
          <p className="mt-5 max-w-2xl text-lg leading-8 text-zinc-300">
            Desktop foundation for tracking AI coding-tool token usage across
            local collectors.
          </p>
        </div>

        <div className="mt-12 grid gap-4 md:grid-cols-3">
          {renderCards(state)}
        </div>
      </section>
    </main>
  );
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
        value="Unavailable"
        detail={state.message}
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
