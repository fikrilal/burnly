import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  getSettings,
  updateProjectPathRetention,
  updateSettings,
} from "../../ipc/client";
import type { AppCapabilitiesResponse } from "../../ipc/generated/contracts";
import { SettingsView } from "./SettingsView";

vi.mock("../../ipc/client", () => ({
  getSettings: vi.fn(),
  updateProjectPathRetention: vi.fn(),
  updateSettings: vi.fn(),
}));

vi.mock("../../ipc/events", () => ({
  EVENT_NAMES: { settingsChanged: "burnly://v1/settings-changed" },
  subscribeToEvent: vi.fn().mockResolvedValue(() => undefined),
}));

const settings = {
  reportingTimezone: "UTC",
  backgroundRefreshEnabled: false,
  refreshIntervalMinutes: 15,
  launchAtLogin: false,
  closeBehavior: "quit" as const,
  notificationsEnabled: false,
  storeProjectPaths: false,
  revision: 1,
};

describe("SettingsView", () => {
  beforeEach(() => {
    setupMocks();
  });

  it("loads dedicated settings and submits the expected revision", async () => {
    const user = userEvent.setup();
    render(<SettingsView capabilities={capabilities()} />, {
      wrapper: queryWrapper(),
    });

    const timezone = await screen.findByLabelText("Reporting timezone");
    await user.clear(timezone);
    await user.type(timezone, "Asia/Jakarta");
    await user.click(screen.getByRole("button", { name: "Save settings" }));

    expect(updateSettings).toHaveBeenCalledWith({
      expectedRevision: 1,
      reportingTimezone: "Asia/Jakarta",
      backgroundRefreshEnabled: false,
      refreshIntervalMinutes: 15,
      launchAtLogin: false,
      closeBehavior: "quit",
      notificationsEnabled: false,
      storeProjectPaths: false,
    });
    expect(await screen.findByText("Settings saved.")).toBeInTheDocument();
  });

  it("submits the native notification preference when supported", async () => {
    const user = userEvent.setup();
    render(<SettingsView capabilities={capabilities(true)} />, {
      wrapper: queryWrapper(),
    });

    await user.click(await screen.findByLabelText("Native notifications"));
    await user.click(screen.getByRole("button", { name: "Save settings" }));

    expect(updateSettings).toHaveBeenCalledWith(
      expect.objectContaining({ notificationsEnabled: true }),
    );
    expect(screen.getByText("Permission: granted")).toBeInTheDocument();
  });
});

describe("SettingsView project-path privacy", () => {
  beforeEach(() => {
    setupMocks();
  });

  it("enables retention prospectively", async () => {
    const user = userEvent.setup();
    render(<SettingsView capabilities={capabilities()} />, {
      wrapper: queryWrapper(),
    });

    await screen.findByLabelText("Reporting timezone");
    await user.click(screen.getByRole("button", { name: "Enable" }));

    expect(updateProjectPathRetention).toHaveBeenCalledWith({
      expectedRevision: 1,
      retainPaths: true,
    });
  });

  it("requires confirmation before deletion", async () => {
    const user = userEvent.setup();
    vi.mocked(getSettings).mockResolvedValue({
      data: { ...settings, storeProjectPaths: true },
      meta: {
        contractVersion: 1,
        requestId: "request-4",
        generatedAt: "2026-06-18T00:00:03.000Z",
      },
    });
    render(<SettingsView capabilities={capabilities()} />, {
      wrapper: queryWrapper(),
    });

    await user.click(await screen.findByRole("button", { name: "Disable" }));
    expect(
      screen.getByText("Remove stored project paths?"),
    ).toBeInTheDocument();
    expect(updateProjectPathRetention).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Remove paths" }));
    expect(updateProjectPathRetention).toHaveBeenCalledWith({
      expectedRevision: 1,
      retainPaths: false,
    });
  });
});

function setupMocks() {
  vi.clearAllMocks();
  vi.mocked(getSettings).mockResolvedValue({
    data: settings,
    meta: {
      contractVersion: 1,
      requestId: "request-1",
      generatedAt: "2026-06-18T00:00:00.000Z",
    },
  });
  vi.mocked(updateSettings).mockResolvedValue({
    data: { ...settings, reportingTimezone: "Asia/Jakarta", revision: 2 },
    meta: {
      contractVersion: 1,
      requestId: "request-2",
      generatedAt: "2026-06-18T00:00:01.000Z",
    },
  });
  vi.mocked(updateProjectPathRetention).mockResolvedValue({
    data: {
      settings: { ...settings, storeProjectPaths: true, revision: 2 },
      clearedPaths: 0,
    },
    meta: {
      contractVersion: 1,
      requestId: "request-3",
      generatedAt: "2026-06-18T00:00:02.000Z",
    },
  });
}

function capabilities(notificationsSupported = false): AppCapabilitiesResponse {
  const unavailable = {
    supported: false,
    status: "not_implemented" as const,
  };
  return {
    tray: unavailable,
    launchAtLogin: unavailable,
    nativeNotifications: notificationsSupported
      ? { supported: true, status: "available", permission: "granted" }
      : { ...unavailable, permission: "unknown" },
    updates: unavailable,
    exportFormats: [],
    diagnostics: { desktopEvidence: true },
  };
}

function queryWrapper() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
  };
}
