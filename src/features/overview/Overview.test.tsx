/* eslint-disable max-lines-per-function */
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi, beforeEach } from "vitest";

import { Overview } from "./Overview";
import {
  getCurrentBudgetProgress,
  getUsageOverview,
  requestRefresh,
  type CommandResult,
} from "../../ipc/client";
import { subscribeToEvent } from "../../ipc/events";
import type {
  RefreshStatusResponse,
  CurrentBudgetProgressResponse,
  UsageOverviewResponse,
} from "../../ipc/generated/contracts";
import { createTestQueryWrapper } from "../../test/query";

vi.mock("../../ipc/client");
vi.mock("../../ipc/events");

const mockOverviewData: UsageOverviewResponse = {
  period: {
    startDate: "2026-05-16",
    endDate: "2026-06-15",
    reportingTimezone: "UTC",
  },
  totalTokens: "1500000",
  activeDays: 14,
  cost: {
    amountMicros: "3500000",
    currency: "USD",
    valuation: "estimated",
    completeness: "partial",
    unavailableDays: 1,
  },
  sources: [
    {
      source: "claude-daily",
      totalTokens: "1000000",
      activeDays: 10,
      cost: {
        amountMicros: "2500000",
        currency: "USD",
        valuation: "available",
        completeness: "complete",
        unavailableDays: 0,
      },
      hasPartialData: false,
    },
    {
      source: "openai-api",
      totalTokens: "500000",
      activeDays: 4,
      cost: {
        amountMicros: "1000000",
        currency: "USD",
        valuation: "estimated",
        completeness: "partial",
        unavailableDays: 0,
      },
      hasPartialData: true,
    },
  ],
  models: [
    {
      name: "Claude 3.5 Sonnet",
      totalTokens: "1000000",
      cost: {
        amountMicros: "2500000",
        currency: "USD",
        valuation: "available",
        completeness: "complete",
        unavailableDays: 0,
      },
    },
  ],
  asOf: "2026-06-15T12:00:00Z",
  lastSuccessfulRefreshAt: "2026-06-15T10:00:00Z",
  dataStatus: "current",
};

const responseMeta = {
  requestId: "1",
  contractVersion: 1,
  generatedAt: "2026-06-15T00:00:00Z",
} as const;

function overviewResult(
  data: UsageOverviewResponse = mockOverviewData,
): CommandResult<UsageOverviewResponse> {
  return { data, meta: responseMeta };
}

function refreshResult(
  data: RefreshStatusResponse,
): CommandResult<RefreshStatusResponse> {
  return { data, meta: responseMeta };
}

function budgetProgressResult(
  data: CurrentBudgetProgressResponse = mockBudgetProgress,
): CommandResult<CurrentBudgetProgressResponse> {
  return { data, meta: responseMeta };
}

const mockBudgetProgress: CurrentBudgetProgressResponse = {
  status: "available",
  reportingTimezone: "UTC",
  asOf: "2026-06-15T12:00:00Z",
  configuredBudgetCount: 1,
  enabledBudgetCount: 1,
  traySummary: "Budget: Monthly token cap 75%",
  items: [
    {
      budgetId: "7",
      budgetName: "Monthly token cap",
      period: "monthly",
      periodStartDate: "2026-06-01",
      periodEndDate: "2026-06-30",
      metric: "tokens",
      state: "available",
      current: "1500000",
      limit: "2000000",
      currency: null,
      basisPoints: "7500",
      exceeded: false,
      completeness: "complete",
      unavailableDays: 0,
    },
  ],
};

describe("Overview Component", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(subscribeToEvent).mockResolvedValue(vi.fn());
    vi.mocked(getCurrentBudgetProgress).mockResolvedValue(
      budgetProgressResult(),
    );
  });

  it("renders loading state initially", () => {
    vi.mocked(getUsageOverview).mockImplementation(
      () =>
        new Promise((resolve) => {
          setTimeout(resolve, 100000);
        }),
    );

    render(<Overview reportingTimezone="UTC" />, {
      wrapper: createTestQueryWrapper(),
    });
    expect(screen.getByText("Loading overview...")).toBeInTheDocument();
  });

  it("renders error state when fetch fails", async () => {
    vi.mocked(getUsageOverview).mockRejectedValue(new Error("Network Error"));

    render(<Overview reportingTimezone="UTC" />, {
      wrapper: createTestQueryWrapper(),
    });

    await waitFor(() => {
      expect(
        screen.getByText("Failed to load overview data"),
      ).toBeInTheDocument();
    });
    expect(
      screen.getByText("Burnly could not load overview data. Try again."),
    ).toBeInTheDocument();
  });

  it("renders populated dashboard successfully", async () => {
    vi.mocked(getUsageOverview).mockResolvedValue(overviewResult());

    render(<Overview reportingTimezone="UTC" />, {
      wrapper: createTestQueryWrapper(),
    });

    await waitFor(() => {
      expect(screen.getByText("Overview")).toBeInTheDocument();
    });

    // Check summary metrics
    expect(screen.getByText("Total Tokens")).toBeInTheDocument();
    expect(screen.getByText("1,500,000")).toBeInTheDocument();
    expect(screen.getAllByText("Cost").length).toBeGreaterThan(0);
    expect(screen.getByText("USD 3.50")).toBeInTheDocument();
    expect(screen.getAllByText("Active Days").length).toBeGreaterThan(0);
    expect(screen.getAllByText("14").length).toBeGreaterThan(0);
    expect(
      screen.getByText("estimated · 1 unavailable day"),
    ).toBeInTheDocument();

    // Check source list
    expect(screen.getByText("claude-daily")).toBeInTheDocument();
    expect(screen.getAllByText("1,000,000")[0]).toBeInTheDocument();
    expect(screen.getAllByText("USD 2.50")[0]).toBeInTheDocument();

    expect(screen.getByText("openai-api")).toBeInTheDocument();
    expect(screen.getByText("500,000")).toBeInTheDocument();
    expect(screen.getByText("USD 1.00")).toBeInTheDocument();
    expect(screen.getByText("Partial")).toBeInTheDocument();

    // Check model list
    expect(screen.getByText("Models")).toBeInTheDocument();
    expect(screen.getByText("Claude 3.5 Sonnet")).toBeInTheDocument();
    expect(screen.getAllByText("1,000,000").length).toBe(2);
    expect(screen.getAllByText("USD 2.50").length).toBe(2);

    // Check refresh control
    expect(screen.getByText("current")).toBeInTheDocument();
    expect(screen.getByText(/Last updated:/i)).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Refresh Now" }),
    ).toBeInTheDocument();
  });

  it("invokes manual refresh when refresh button is clicked", async () => {
    const user = userEvent.setup();
    vi.mocked(getUsageOverview).mockResolvedValue(overviewResult());

    vi.mocked(requestRefresh).mockResolvedValue(
      refreshResult({
        status: "idle",
        lastSuccessfulRefreshAt: null,
        jobId: null,
        trigger: "manual",
      }),
    );

    render(<Overview reportingTimezone="UTC" />, {
      wrapper: createTestQueryWrapper(),
    });

    await clickRefresh(user);

    expect(requestRefresh).toHaveBeenCalled();
  });

  it("renders empty state when dataStatus is empty", async () => {
    vi.mocked(getUsageOverview).mockResolvedValue(
      overviewResult({
        ...mockOverviewData,
        dataStatus: "empty",
        totalTokens: "0",
        sources: [],
      }),
    );

    render(<Overview reportingTimezone="UTC" />, {
      wrapper: createTestQueryWrapper(),
    });

    await waitFor(() => {
      expect(screen.getByText("No data collected")).toBeInTheDocument();
    });

    expect(screen.queryByText("Total Tokens")).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Refresh Data" }),
    ).toBeInTheDocument();
  });

  it("calls refetch when retry button is clicked", async () => {
    const user = userEvent.setup();
    vi.mocked(getUsageOverview).mockRejectedValueOnce(new Error("First Error"));
    vi.mocked(getUsageOverview).mockResolvedValueOnce(overviewResult());

    render(<Overview reportingTimezone="UTC" />, {
      wrapper: createTestQueryWrapper(),
    });

    await waitFor(() => {
      expect(
        screen.getByText("Failed to load overview data"),
      ).toBeInTheDocument();
    });

    const retryButton = screen.getByRole("button", { name: "Retry" });
    await user.click(retryButton);

    await waitFor(() => {
      expect(screen.getByText("1,500,000")).toBeInTheDocument();
    });
  });

  it("shows refresh error banner but preserves prior data when refresh fails", async () => {
    const user = userEvent.setup();
    vi.mocked(getUsageOverview).mockResolvedValue(overviewResult());

    vi.mocked(requestRefresh).mockRejectedValue(
      new Error("Refresh failed to connect"),
    );

    render(<Overview reportingTimezone="UTC" />, {
      wrapper: createTestQueryWrapper(),
    });

    await clickRefresh(user);

    await waitFor(() => {
      expect(screen.getByText("Refresh failed")).toBeInTheDocument();
    });

    expect(
      screen.getByText("Displaying the last successful overview."),
    ).toBeInTheDocument();
    // Prior data should still be visible
    expect(screen.getByText("1,500,000")).toBeInTheDocument();
  });

  it("uses the configured reporting timezone", async () => {
    vi.mocked(getUsageOverview).mockResolvedValue(
      overviewResult({
        ...mockOverviewData,
        period: {
          ...mockOverviewData.period,
          reportingTimezone: "Asia/Jakarta",
        },
      }),
    );

    render(<Overview reportingTimezone="Asia/Jakarta" />, {
      wrapper: createTestQueryWrapper(),
    });

    await waitFor(() => {
      expect(getUsageOverview).toHaveBeenCalledWith(
        expect.objectContaining({ reportingTimezone: "Asia/Jakarta" }),
      );
    });
  });

  it("preserves prior data when an invalidation re-query fails", async () => {
    const invalidationCallbacks: (() => void)[] = [];

    vi.mocked(subscribeToEvent).mockImplementation((event, callback) => {
      if (event === "burnly://v1/data-invalidated") {
        invalidationCallbacks.push(() => {
          callback({});
        });
      }
      return Promise.resolve(vi.fn());
    });

    vi.mocked(getUsageOverview)
      .mockResolvedValueOnce(overviewResult())
      .mockRejectedValueOnce(new Error("database is locked"));

    render(<Overview reportingTimezone="UTC" />, {
      wrapper: createTestQueryWrapper(),
    });

    await waitFor(() => {
      expect(screen.getByText("1,500,000")).toBeInTheDocument();
    });

    invalidationCallbacks.forEach((callback) => {
      callback();
    });

    await waitFor(() => {
      expect(screen.getByText("Overview update failed")).toBeInTheDocument();
    });

    expect(screen.getByText("1,500,000")).toBeInTheDocument();
    expect(screen.queryByText("database is locked")).not.toBeInTheDocument();
  });

  it("renders refresh progress from events", async () => {
    let progress: ((payload: Record<string, unknown>) => void) | undefined;

    vi.mocked(subscribeToEvent).mockImplementation((event, callback) => {
      if (event === "burnly://v1/refresh-progress") {
        progress = callback;
      }
      return Promise.resolve(vi.fn());
    });

    vi.mocked(getUsageOverview).mockResolvedValue(overviewResult());

    render(<Overview reportingTimezone="UTC" />, {
      wrapper: createTestQueryWrapper(),
    });

    await waitFor(() => {
      expect(screen.getByText("1,500,000")).toBeInTheDocument();
    });

    progress?.({ status: "running" });

    await waitFor(() => {
      expect(screen.getByText("Refresh running")).toBeInTheDocument();
    });
  });
});

async function clickRefresh(user: ReturnType<typeof userEvent.setup>) {
  await waitFor(() => {
    expect(
      screen.getByRole("button", { name: "Refresh Now" }),
    ).toBeInTheDocument();
  });

  await user.click(screen.getByRole("button", { name: "Refresh Now" }));
}
