import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { getSettings, updateSettings } from "../../ipc/client";
import type { AppCapabilitiesResponse } from "../../ipc/generated/contracts";
import { SettingsView } from "./SettingsView";

vi.mock("../../ipc/client", () => ({
  getSettings: vi.fn(),
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

  it("shows unavailable platform-owned settings as read-only", async () => {
    render(<SettingsView capabilities={capabilities()} />, {
      wrapper: queryWrapper(),
    });

    await screen.findByLabelText("Reporting timezone");
    expect(screen.getAllByText("Unavailable")).toHaveLength(3);
  });
});

function capabilities(): AppCapabilitiesResponse {
  const unavailable = {
    supported: false,
    status: "not_implemented" as const,
  };
  return {
    tray: unavailable,
    launchAtLogin: unavailable,
    nativeNotifications: unavailable,
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
