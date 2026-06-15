import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { getUsageOverview, requestRefresh } from "../../ipc/client";
import { EVENT_NAMES, subscribeToEvent } from "../../ipc/events";
import { useOverview } from "./use-overview";
import type {
  UsageOverviewRequest,
  UsageOverviewResponse,
  EventName,
  RefreshStatusResponse,
  UnknownEventPayload,
} from "../../ipc/generated/contracts";

vi.mock("../../ipc/client");
vi.mock("../../ipc/events");

const mockRequest: UsageOverviewRequest = {
  startDate: "2026-06-13",
  endDate: "2026-06-14",
  reportingTimezone: "UTC",
};

const createMockOverview = (tokens: string): UsageOverviewResponse => ({
  period: mockRequest,
  totalTokens: tokens,
  activeDays: 1,
  cost: {
    amountMicros: null,
    currency: null,
    valuation: "unavailable",
    completeness: "unavailable",
    unavailableDays: 1,
  },
  sources: [],
  asOf: "2026-06-15T00:00:00Z",
  lastSuccessfulRefreshAt: null,
  dataStatus: "current",
});

const mockRefreshStatus: RefreshStatusResponse = {
  status: "idle",
  lastSuccessfulRefreshAt: null,
  jobId: null,
  trigger: "manual",
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

describe("useOverview", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("fetches overview data on mount", async () => {
    const mockData = createMockOverview("123");
    vi.mocked(getUsageOverview).mockResolvedValueOnce({
      data: mockData,
      meta: {
        requestId: "1",
        contractVersion: 1,
        generatedAt: "2026-06-15T00:00:00Z",
      },
    });

    vi.mocked(subscribeToEvent).mockResolvedValueOnce(vi.fn());

    const { result } = renderHook(() => useOverview(mockRequest), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.isSuccess).toBe(true);
    });

    expect(result.current.data).toEqual(mockData);
    expect(getUsageOverview).toHaveBeenCalledWith(mockRequest);
  });

  it("subscribes to dataInvalidated event and invalidates query", async () => {
    let mockCallback: (() => void) | undefined;

    vi.mocked(subscribeToEvent).mockImplementation(
      (event: EventName, callback: (payload: UnknownEventPayload) => void) => {
        if (event === EVENT_NAMES.dataInvalidated) {
          mockCallback = () => {
            callback({});
          };
        }
        return Promise.resolve(vi.fn());
      },
    );

    const mockData1 = createMockOverview("1");
    const mockData2 = createMockOverview("2");

    vi.mocked(getUsageOverview)
      .mockResolvedValueOnce({
        data: mockData1,
        meta: {
          requestId: "1",
          contractVersion: 1,
          generatedAt: "2026-06-15T00:00:00Z",
        },
      })
      .mockResolvedValueOnce({
        data: mockData2,
        meta: {
          requestId: "2",
          contractVersion: 1,
          generatedAt: "2026-06-15T00:00:00Z",
        },
      });

    const { result } = renderHook(() => useOverview(mockRequest), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.data).toEqual(mockData1);
    });

    expect(mockCallback).toBeDefined();

    // Trigger invalidation
    if (mockCallback) {
      mockCallback();
    }

    await waitFor(() => {
      expect(result.current.data).toEqual(mockData2);
    });
  });

  it("invokes manual refresh", async () => {
    vi.mocked(getUsageOverview).mockResolvedValueOnce({
      data: createMockOverview("1"),
      meta: {
        requestId: "1",
        contractVersion: 1,
        generatedAt: "2026-06-15T00:00:00Z",
      },
    });
    vi.mocked(subscribeToEvent).mockResolvedValueOnce(vi.fn());
    vi.mocked(requestRefresh).mockResolvedValueOnce({
      data: mockRefreshStatus,
      meta: {
        requestId: "1",
        contractVersion: 1,
        generatedAt: "2026-06-15T00:00:00Z",
      },
    });

    const { result } = renderHook(() => useOverview(mockRequest), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.isSuccess).toBe(true);
    });

    await result.current.manualRefresh();

    expect(requestRefresh).toHaveBeenCalled();
  });
});
