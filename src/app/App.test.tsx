import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { App } from "./App";
import type { CommandResult } from "../ipc/client";
import { BurnlyClientError } from "../ipc/errors";
import { CONTRACT_VERSION } from "../ipc/generated/contracts";
import type {
  AppBootstrapResponse,
  AppCapabilitiesResponse,
} from "../ipc/generated/contracts";

vi.mock("../features/overview", () => ({
  Overview: () => <div data-testid="overview-feature" />,
}));

vi.mock("../features/diagnostics", () => ({
  DiagnosticsView: () => <div data-testid="diagnostics-feature" />,
}));

vi.mock("../ipc/events", () => ({
  EVENT_NAMES: {
    settingsChanged: "burnly://v1/settings-changed",
  },
  subscribeToEvent: vi.fn().mockResolvedValue(() => {
    /* no-op */
  }),
}));

const meta = {
  contractVersion: CONTRACT_VERSION,
  requestId: "018f5f4d-7758-7bb2-9d9b-6d7f22c4a901",
  generatedAt: "2026-06-14T07:30:00.000Z",
} as const;

describe("App", () => {
  it("renders the Overview feature when the runtime is ready", async () => {
    render(
      <App
        loadBootstrap={() => Promise.resolve(bootstrapResult())}
        loadCapabilities={() => Promise.resolve(capabilitiesResult())}
      />,
    );

    expect(await screen.findByTestId("overview-feature")).toBeInTheDocument();
  });

  it("opens diagnostics from the main navigation", async () => {
    const user = userEvent.setup();

    render(
      <App
        loadBootstrap={() => Promise.resolve(bootstrapResult())}
        loadCapabilities={() => Promise.resolve(capabilitiesResult())}
      />,
    );

    await user.click(
      await screen.findByRole("button", { name: "Diagnostics" }),
    );

    expect(screen.getByTestId("diagnostics-feature")).toBeInTheDocument();
  });

  it("stops startup before capability loading when contract versions differ", async () => {
    const loadCapabilities =
      vi.fn<() => Promise<CommandResult<AppCapabilitiesResponse>>>();

    render(
      <App
        loadBootstrap={() => Promise.resolve(bootstrapResult(2))}
        loadCapabilities={loadCapabilities}
      />,
    );

    expect(await screen.findByText("Incompatible")).toBeInTheDocument();
    expect(screen.getByText("Frontend v1, runtime v2")).toBeInTheDocument();
    expect(loadCapabilities).not.toHaveBeenCalled();
  });
});

describe("App startup failures", () => {
  it("renders a failure state when runtime state cannot be loaded", async () => {
    render(
      <App
        loadBootstrap={() => Promise.reject(new Error("runtime offline"))}
        loadCapabilities={() => Promise.reject(new Error("runtime offline"))}
      />,
    );

    expect(await screen.findByText("Runtime unavailable")).toBeInTheDocument();
    expect(screen.getByText("runtime offline")).toBeInTheDocument();
  });

  it("renders expected application errors separately from transport failures", async () => {
    render(
      <App
        loadBootstrap={() =>
          Promise.reject(
            new BurnlyClientError({
              kind: "application",
              error: {
                code: "bootstrap.storage_unavailable",
                message: "Burnly could not read local application state.",
                category: "persistence",
                retryable: true,
                details: null,
              },
              requestId: meta.requestId,
              generatedAt: meta.generatedAt,
            }),
          )
        }
        loadCapabilities={() => Promise.resolve(capabilitiesResult())}
      />,
    );

    expect(await screen.findByText("Application error")).toBeInTheDocument();
    expect(
      screen.getByText("Burnly could not read local application state."),
    ).toBeInTheDocument();
  });
});

function bootstrapResult(
  contractVersion: typeof CONTRACT_VERSION | 2 = CONTRACT_VERSION,
): CommandResult<AppBootstrapResponse> {
  return {
    data: {
      appVersion: "0.1.0",
      contractVersion,
      database: {
        status: "ready",
        schemaVersion: 1,
      },
      settings: {
        reportingTimezone: "Asia/Jakarta",
        backgroundRefreshEnabled: false,
        refreshIntervalMinutes: 15,
        launchAtLogin: false,
        closeBehavior: "quit",
        notificationsEnabled: false,
        storeProjectPaths: false,
        revision: 1,
      },
      features: {
        usageOverview: false,
        collectorRefresh: false,
        budgets: false,
        settings: false,
      },
      sources: {
        status: "not_configured",
        detectedCount: 0,
        configuredCount: 0,
        enabledCount: 0,
      },
      refresh: {
        status: "idle",
        currentJobId: null,
        lastSuccessfulRefreshAt: null,
      },
      onboardingComplete: false,
    },
    meta,
  };
}

function capabilitiesResult(): CommandResult<AppCapabilitiesResponse> {
  const capability = {
    supported: false,
    status: "not_implemented",
  } as const;

  return {
    data: {
      tray: capability,
      launchAtLogin: capability,
      nativeNotifications: { ...capability, permission: "unknown" },
      updates: capability,
      exportFormats: [],
      diagnostics: {
        desktopEvidence: true,
      },
    },
    meta,
  };
}
