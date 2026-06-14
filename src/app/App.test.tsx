import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { App } from "./App";
import type { CommandResult } from "../ipc/client";
import { CONTRACT_VERSION } from "../ipc/generated/contracts";
import type {
  AppBootstrapResponse,
  AppCapabilitiesResponse,
} from "../ipc/generated/contracts";

const meta = {
  contractVersion: CONTRACT_VERSION,
  requestId: "018f5f4d-7758-7bb2-9d9b-6d7f22c4a901",
  generatedAt: "2026-06-14T07:30:00.000Z",
} as const;

describe("App", () => {
  it("renders bootstrap and capability data from the IPC client boundary", async () => {
    render(
      <App
        loadBootstrap={() => Promise.resolve(bootstrapResult())}
        loadCapabilities={() => Promise.resolve(capabilitiesResult())}
      />,
    );

    expect(
      screen.getByRole("heading", { level: 1, name: "Burnly" }),
    ).toBeInTheDocument();
    expect(await screen.findByText("Schema 1")).toBeInTheDocument();
    expect(screen.getByText("Asia/Jakarta")).toBeInTheDocument();
    expect(screen.getByText("not implemented")).toBeInTheDocument();
  });

  it("renders a failure state when runtime state cannot be loaded", async () => {
    render(
      <App
        loadBootstrap={() => Promise.reject(new Error("runtime offline"))}
        loadCapabilities={() => Promise.reject(new Error("runtime offline"))}
      />,
    );

    expect(await screen.findByText("Unavailable")).toBeInTheDocument();
    expect(screen.getByText("runtime offline")).toBeInTheDocument();
  });
});

function bootstrapResult(): CommandResult<AppBootstrapResponse> {
  return {
    data: {
      appVersion: "0.1.0",
      contractVersion: CONTRACT_VERSION,
      database: {
        status: "ready",
        schemaVersion: 1,
      },
      settings: {
        reportingTimezone: "Asia/Jakarta",
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
      nativeNotifications: capability,
      updates: capability,
      exportFormats: [],
      diagnostics: {
        desktopEvidence: true,
      },
    },
    meta,
  };
}
