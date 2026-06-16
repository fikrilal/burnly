/* eslint-disable max-lines-per-function */
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { describe, expect, it, vi, beforeEach } from "vitest";

import { Overview } from "./Overview";
import { getUsageOverview, requestRefresh } from "../../ipc/client";
import { subscribeToEvent } from "../../ipc/events";
import type { UsageOverviewResponse } from "../../ipc/generated/contracts";

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
    unavailableDays: 0,
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

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    );
  };
}

describe("Overview Component", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(subscribeToEvent).mockResolvedValue(vi.fn());
  });

  it("renders loading state initially", () => {
    vi.mocked(getUsageOverview).mockImplementation(
      () =>
        new Promise((resolve) => {
          setTimeout(resolve, 100000);
        }),
    );

    render(<Overview />, { wrapper: createWrapper() });
    expect(screen.getByText("Loading overview...")).toBeInTheDocument();
  });

  it("renders error state when fetch fails", async () => {
    vi.mocked(getUsageOverview).mockRejectedValue(new Error("Network Error"));

    render(<Overview />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(
        screen.getByText("Failed to load overview data"),
      ).toBeInTheDocument();
    });
    expect(screen.getByText("Error: Network Error")).toBeInTheDocument();
  });

  it("renders populated dashboard successfully", async () => {
    vi.mocked(getUsageOverview).mockResolvedValue({
      data: mockOverviewData,
      meta: {
        requestId: "1",
        contractVersion: 1,
        generatedAt: "2026-06-15T00:00:00Z",
      },
    });

    render(<Overview />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText("Overview")).toBeInTheDocument();
    });

    // Check summary metrics
    expect(screen.getByText("Total Tokens")).toBeInTheDocument();
    expect(screen.getByText("1,500,000")).toBeInTheDocument();
    expect(screen.getByText("Estimated Cost")).toBeInTheDocument();
    expect(screen.getByText("$3.50")).toBeInTheDocument();
    expect(screen.getAllByText("Active Days").length).toBeGreaterThan(0);
    expect(screen.getAllByText("14").length).toBeGreaterThan(0);
    expect(screen.getByText("Cost data estimated")).toBeInTheDocument();

    // Check source list
    expect(screen.getByText("claude-daily")).toBeInTheDocument();
    expect(screen.getByText("1,000,000")).toBeInTheDocument();
    expect(screen.getByText("$2.50")).toBeInTheDocument();

    expect(screen.getByText("openai-api")).toBeInTheDocument();
    expect(screen.getByText("500,000")).toBeInTheDocument();
    expect(screen.getByText("$1.00")).toBeInTheDocument();
    expect(screen.getByText("Partial")).toBeInTheDocument();

    // Check model list
    expect(screen.getByText("Models")).toBeInTheDocument();
    expect(screen.getByText("Claude 3.5 Sonnet")).toBeInTheDocument();
    expect(screen.getAllByText("1,000,000").length).toBe(2);
    expect(screen.getAllByText("$2.50").length).toBe(2);

    // Check refresh control
    expect(screen.getByText("current")).toBeInTheDocument();
    expect(screen.getByText(/Last updated:/i)).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Refresh Now" }),
    ).toBeInTheDocument();
  });

  it("invokes manual refresh when refresh button is clicked", async () => {
    const user = userEvent.setup();
    vi.mocked(getUsageOverview).mockResolvedValue({
      data: mockOverviewData,
      meta: {
        requestId: "1",
        contractVersion: 1,
        generatedAt: "2026-06-15T00:00:00Z",
      },
    });

    vi.mocked(requestRefresh).mockResolvedValue({
      data: {
        status: "idle",
        lastSuccessfulRefreshAt: null,
        jobId: null,
        trigger: "manual",
      },
      meta: {
        requestId: "1",
        contractVersion: 1,
        generatedAt: "2026-06-15T00:00:00Z",
      },
    });

    render(<Overview />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: "Refresh Now" }),
      ).toBeInTheDocument();
    });

    await user.click(screen.getByRole("button", { name: "Refresh Now" }));

    expect(requestRefresh).toHaveBeenCalled();
  });

  it("renders empty state when dataStatus is empty", async () => {
    vi.mocked(getUsageOverview).mockResolvedValue({
      data: {
        ...mockOverviewData,
        dataStatus: "empty",
        totalTokens: "0",
        sources: [],
      },
      meta: {
        requestId: "1",
        contractVersion: 1,
        generatedAt: "2026-06-15T00:00:00Z",
      },
    });

    render(<Overview />, { wrapper: createWrapper() });

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
    vi.mocked(getUsageOverview).mockResolvedValueOnce({
      data: mockOverviewData,
      meta: {
        requestId: "1",
        contractVersion: 1,
        generatedAt: "2026-06-15T00:00:00Z",
      },
    });

    render(<Overview />, { wrapper: createWrapper() });

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
    vi.mocked(getUsageOverview).mockResolvedValue({
      data: mockOverviewData,
      meta: {
        requestId: "1",
        contractVersion: 1,
        generatedAt: "2026-06-15T00:00:00Z",
      },
    });

    vi.mocked(requestRefresh).mockRejectedValue(
      new Error("Refresh failed to connect"),
    );

    render(<Overview />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: "Refresh Now" }),
      ).toBeInTheDocument();
    });

    await user.click(screen.getByRole("button", { name: "Refresh Now" }));

    await waitFor(() => {
      expect(screen.getByText("Refresh Failed")).toBeInTheDocument();
    });

    expect(screen.getByText(/Refresh failed to connect/)).toBeInTheDocument();
    // Prior data should still be visible
    expect(screen.getByText("1,500,000")).toBeInTheDocument();
  });
});
