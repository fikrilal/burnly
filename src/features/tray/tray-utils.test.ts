import { describe, expect, it } from "vitest";

import type { TraySummaryResponse } from "../../ipc/generated/contracts";
import { trayHeaderStatus } from "./tray-utils";

type DataStatus = TraySummaryResponse["dataStatus"];
type DataQuality = TraySummaryResponse["dataQuality"];
type LatestRefreshStatus = TraySummaryResponse["latestRefreshStatus"];

describe("trayHeaderStatus", () => {
  const base = {
    dataStatus: "current" as DataStatus,
    dataQuality: "complete" as DataQuality,
    latestRefreshStatus: "succeeded" as LatestRefreshStatus,
    isRefreshing: false,
    isError: false,
  };

  it("keeps the normal relative-update presentation for current data", () => {
    expect(trayHeaderStatus(base)).toBe("current");
  });

  it("maps empty data availability to the empty state", () => {
    expect(
      trayHeaderStatus({
        ...base,
        dataStatus: "empty",
        latestRefreshStatus: null,
      }),
    ).toBe("empty");
  });

  it("derives estimated usage from partial data quality alone", () => {
    expect(trayHeaderStatus({ ...base, dataQuality: "partial" })).toBe(
      "estimated",
    );
  });

  it("reserves the partial state for a partial refresh", () => {
    expect(trayHeaderStatus({ ...base, latestRefreshStatus: "partial" })).toBe(
      "partial",
    );
  });

  it("gives a partial refresh precedence over estimated usage", () => {
    expect(
      trayHeaderStatus({
        ...base,
        dataQuality: "partial",
        latestRefreshStatus: "partial",
      }),
    ).toBe("partial");
  });

  it("maps a failed latest refresh to the failed state", () => {
    expect(trayHeaderStatus({ ...base, latestRefreshStatus: "failed" })).toBe(
      "failed",
    );
  });

  it("maps a cancelled latest refresh to the failed state", () => {
    expect(
      trayHeaderStatus({
        ...base,
        dataQuality: "partial",
        latestRefreshStatus: "cancelled",
      }),
    ).toBe("failed");
  });

  it("maps an active refresh to the refreshing state", () => {
    expect(trayHeaderStatus({ ...base, isRefreshing: true })).toBe(
      "refreshing",
    );
  });

  it("gives an active refresh precedence over a failed latest refresh", () => {
    expect(
      trayHeaderStatus({
        ...base,
        latestRefreshStatus: "failed",
        isRefreshing: true,
      }),
    ).toBe("refreshing");
  });

  it("maps a query error to the failed state", () => {
    expect(trayHeaderStatus({ ...base, isError: true })).toBe("failed");
  });

  it("gives a query error precedence over every other condition", () => {
    expect(
      trayHeaderStatus({
        ...base,
        dataQuality: "partial",
        latestRefreshStatus: "partial",
        isRefreshing: true,
        isError: true,
      }),
    ).toBe("failed");
  });
});
