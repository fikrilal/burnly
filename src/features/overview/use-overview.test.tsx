import { renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  getUsageOverview,
  requestRefresh,
  type CommandResult,
} from "../../ipc/client";
import { EVENT_NAMES, subscribeToEvent } from "../../ipc/events";
import { useOverview } from "./use-overview";
import type {
  UsageOverviewRequest,
  UsageOverviewResponse,
  EventName,
  RefreshStatusResponse,
  UnknownEventPayload,
} from "../../ipc/generated/contracts";
import { createTestQueryWrapper } from "../../test/query";

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
  models: [],
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

const responseMeta = {
  requestId: "1",
  contractVersion: 1,
  generatedAt: "2026-06-15T00:00:00Z",
} as const;

function overviewResult(
  data: UsageOverviewResponse,
): CommandResult<UsageOverviewResponse> {
  return { data, meta: responseMeta };
}

function refreshResult(
  data: RefreshStatusResponse,
): CommandResult<RefreshStatusResponse> {
  return { data, meta: responseMeta };
}

describe("useOverview", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("fetches overview data on mount", async () => {
    const mockData = createMockOverview("123");
    vi.mocked(getUsageOverview).mockResolvedValueOnce(overviewResult(mockData));

    vi.mocked(subscribeToEvent).mockResolvedValue(vi.fn());

    const { result } = renderHook(() => useOverview(mockRequest), {
      wrapper: createTestQueryWrapper(),
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
      .mockResolvedValueOnce(overviewResult(mockData1))
      .mockResolvedValueOnce(overviewResult(mockData2));

    const { result } = renderHook(() => useOverview(mockRequest), {
      wrapper: createTestQueryWrapper(),
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
    vi.mocked(getUsageOverview).mockResolvedValueOnce(
      overviewResult(createMockOverview("1")),
    );
    vi.mocked(subscribeToEvent).mockResolvedValue(vi.fn());
    vi.mocked(requestRefresh).mockResolvedValueOnce(
      refreshResult(mockRefreshStatus),
    );

    const { result } = renderHook(() => useOverview(mockRequest), {
      wrapper: createTestQueryWrapper(),
    });

    await waitFor(() => {
      expect(result.current.isSuccess).toBe(true);
    });

    await result.current.manualRefresh();

    expect(requestRefresh).toHaveBeenCalled();
  });

  it("tracks refresh progress events", async () => {
    let progress: ((payload: UnknownEventPayload) => void) | undefined;

    vi.mocked(subscribeToEvent).mockImplementation(
      (event: EventName, callback: (payload: UnknownEventPayload) => void) => {
        if (event === EVENT_NAMES.refreshProgress) {
          progress = callback;
        }
        return Promise.resolve(vi.fn());
      },
    );

    vi.mocked(getUsageOverview).mockResolvedValueOnce(
      overviewResult(createMockOverview("1")),
    );

    const { result } = renderHook(() => useOverview(mockRequest), {
      wrapper: createTestQueryWrapper(),
    });

    await waitFor(() => {
      expect(result.current.isSuccess).toBe(true);
    });

    progress?.({ status: "running" });

    await waitFor(() => {
      expect(result.current.refreshStatus).toBe("running");
      expect(result.current.isRefreshing).toBe(true);
    });
  });
});
