import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { TrayPanel } from "./TrayPanel";
import {
  getSettings,
  getTraySummary,
  updateSettings,
  type CommandResult,
} from "../../ipc/client";
import { subscribeToEvent } from "../../ipc/events";
import type {
  SettingsResponse,
  TraySummaryResponse,
} from "../../ipc/generated/contracts";
import { createTestQueryWrapper } from "../../test/query";

vi.mock("../../ipc/client");
vi.mock("../../ipc/events");

const responseMeta = {
  requestId: "1",
  contractVersion: 1,
  generatedAt: "2026-06-25T00:00:00Z",
} as const;

const summary: TraySummaryResponse = {
  today: {
    startDate: "2026-06-25",
    endDate: "2026-06-25",
    totalTokens: "42180",
  },
  week: {
    startDate: "2026-06-22",
    endDate: "2026-06-28",
    totalTokens: "183240",
  },
  month: {
    startDate: "2026-06-01",
    endDate: "2026-06-30",
    totalTokens: "612900",
  },
  models: [
    {
      modelName: "GPT-5.1",
      agentLabel: "Codex",
      totalTokens: "25000",
      trend: {
        direction: "increased",
        basisPoints: 850,
      },
    },
    {
      modelName: "Claude Sonnet",
      agentLabel: "Claude Code",
      totalTokens: "12000",
      trend: null,
    },
  ],
  asOf: "2026-06-25T12:00:00Z",
  lastSuccessfulRefreshAt: "2026-06-25T11:55:00Z",
  dataStatus: "current",
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(subscribeToEvent).mockResolvedValue(() => {
    /* no-op */
  });
});

describe("TrayPanel overview", () => {
  it("renders compact token metrics and model allocation", async () => {
    vi.mocked(getTraySummary).mockResolvedValue(traySummaryResult());

    render(<TrayPanel reportingTimezone="Asia/Jakarta" />, {
      wrapper: createTestQueryWrapper(),
    });

    expect(await screen.findByText("42,180")).toBeInTheDocument();
    expect(screen.getByText("183.2K")).toBeInTheDocument();
    expect(screen.getByText("612.9K")).toBeInTheDocument();
    expect(screen.getByText("GPT-5.1")).toBeInTheDocument();
    expect(screen.getByText("Codex")).toBeInTheDocument();
    expect(screen.getByText("8.5%")).toBeInTheDocument();
    expect(screen.getByText("Claude Code")).toBeInTheDocument();
    expect(screen.getByText("new today")).toBeInTheDocument();
    expect(screen.queryByText(/cost/i)).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /refresh/i }),
    ).not.toBeInTheDocument();
  });

  it("renders empty usage without a refresh button", async () => {
    vi.mocked(getTraySummary).mockResolvedValue(
      traySummaryResult({
        ...summary,
        today: { ...summary.today, totalTokens: "0" },
        models: [],
        dataStatus: "empty",
        lastSuccessfulRefreshAt: null,
      }),
    );

    render(<TrayPanel reportingTimezone="Asia/Jakarta" />, {
      wrapper: createTestQueryWrapper(),
    });

    expect(
      await screen.findByText("No usage collected today"),
    ).toBeInTheDocument();
    expect(screen.getByText("No model usage today")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /refresh/i }),
    ).not.toBeInTheDocument();
  });

  it("renders failed loading state", async () => {
    vi.mocked(getTraySummary).mockRejectedValue(new Error("summary offline"));

    render(<TrayPanel reportingTimezone="Asia/Jakarta" />, {
      wrapper: createTestQueryWrapper(),
    });

    expect(await screen.findByText("Failed")).toBeInTheDocument();
    expect(screen.getByText("summary offline")).toBeInTheDocument();
  });
});

describe("TrayPanel settings controls", () => {
  it("renders persisted close behavior in settings", async () => {
    vi.mocked(getTraySummary).mockResolvedValue(traySummaryResult());
    vi.mocked(getSettings).mockResolvedValue(
      settingsResult({ closeBehavior: "quit" }),
    );

    render(<TrayPanel reportingTimezone="Asia/Jakarta" />, {
      wrapper: createTestQueryWrapper(),
    });

    await userEvent.click(
      await screen.findByRole("button", { name: "Settings" }),
    );

    expect(await screen.findByText("Quit on close")).toBeInTheDocument();
    expect(
      screen.getByRole("switch", { name: "Quit on close" }),
    ).toHaveAttribute("aria-checked", "true");
  });

  it("updates close behavior while preserving hidden settings fields", async () => {
    const user = userEvent.setup();
    vi.mocked(getTraySummary).mockResolvedValue(traySummaryResult());
    vi.mocked(getSettings).mockResolvedValue(
      settingsResult({
        launchAtLogin: true,
        closeBehavior: "quit",
        revision: 7,
      }),
    );
    vi.mocked(updateSettings).mockResolvedValue(
      settingsResult({
        launchAtLogin: true,
        closeBehavior: "hide",
        revision: 8,
      }),
    );

    render(<TrayPanel reportingTimezone="Asia/Jakarta" />, {
      wrapper: createTestQueryWrapper(),
    });

    await user.click(await screen.findByRole("button", { name: "Settings" }));
    await user.click(
      await screen.findByRole("switch", { name: "Quit on close" }),
    );

    await waitFor(() => {
      expect(updateSettings).toHaveBeenCalledWith({
        launchAtLogin: true,
        closeBehavior: "hide",
        expectedRevision: 7,
      });
    });
  });
});

describe("TrayPanel settings failures", () => {
  it("renders settings load failures", async () => {
    const user = userEvent.setup();
    vi.mocked(getTraySummary).mockResolvedValue(traySummaryResult());
    vi.mocked(getSettings).mockRejectedValue(new Error("settings offline"));

    render(<TrayPanel reportingTimezone="Asia/Jakarta" />, {
      wrapper: createTestQueryWrapper(),
    });

    await user.click(await screen.findByRole("button", { name: "Settings" }));

    expect(await screen.findByText("Settings unavailable")).toBeInTheDocument();
    expect(screen.getByText("settings offline")).toBeInTheDocument();
  });

  it("renders settings save failures", async () => {
    const user = userEvent.setup();
    vi.mocked(getTraySummary).mockResolvedValue(traySummaryResult());
    vi.mocked(getSettings).mockResolvedValue(
      settingsResult({ closeBehavior: "quit" }),
    );
    vi.mocked(updateSettings).mockRejectedValue(new Error("settings conflict"));

    render(<TrayPanel reportingTimezone="Asia/Jakarta" />, {
      wrapper: createTestQueryWrapper(),
    });

    await user.click(await screen.findByRole("button", { name: "Settings" }));
    await user.click(
      await screen.findByRole("switch", { name: "Quit on close" }),
    );

    expect(await screen.findByText("Settings not saved")).toBeInTheDocument();
    expect(screen.getByText("settings conflict")).toBeInTheDocument();
  });
});

function traySummaryResult(
  data: TraySummaryResponse = summary,
): CommandResult<TraySummaryResponse> {
  return { data, meta: responseMeta };
}

function settingsResult(
  overrides: Partial<SettingsResponse> = {},
): CommandResult<SettingsResponse> {
  return {
    data: {
      launchAtLogin: false,
      closeBehavior: "hide",
      revision: 1,
      ...overrides,
    },
    meta: responseMeta,
  };
}
