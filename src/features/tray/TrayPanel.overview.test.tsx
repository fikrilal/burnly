import { screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { TrayPanel } from "./TrayPanel";
import { getTraySummary } from "../../ipc/client";
import {
  capabilities,
  longModelList,
  renderTrayPanel,
  resetTrayPanelMocks,
  summary,
  traySummaryResult,
} from "./test_support";

vi.mock("../../ipc/client");
vi.mock("../../ipc/external-links");
vi.mock("../../ipc/events");

beforeEach(resetTrayPanelMocks);

describe("TrayPanel overview", () => {
  it("renders compact token metrics and model allocation", async () => {
    vi.mocked(getTraySummary).mockResolvedValue(traySummaryResult());

    renderTrayPanel();

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

  it("keeps overview content in a scrollable tab region", async () => {
    vi.mocked(getTraySummary).mockResolvedValue(
      traySummaryResult({
        ...summary,
        models: longModelList(),
      }),
    );

    renderTrayPanel();

    const overview = await screen.findByRole("region", { name: "Overview" });
    const surface = overview.closest(".tray-surface");

    expect(surface).toHaveClass("h-screen");
    expect(surface).not.toHaveClass("min-h-screen");
    expect(overview).toHaveClass("overflow-y-auto");
    expect(overview).toHaveClass("tray-scroll-area");
    expect(screen.getByText("Model 12")).toBeInTheDocument();
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

    renderTrayPanel();

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

    renderTrayPanel();

    expect(await screen.findByText("Refresh failed")).toBeInTheDocument();
    expect(screen.getByText("summary offline")).toBeInTheDocument();
  });

  it("reports estimated usage without claiming a source failed", async () => {
    vi.mocked(getTraySummary).mockResolvedValue(
      traySummaryResult({ ...summary, dataQuality: "partial" }),
    );

    renderTrayPanel();

    expect(
      await screen.findByText("Some usage is estimated"),
    ).toBeInTheDocument();
    expect(screen.queryByText("Some sources failed")).not.toBeInTheDocument();
    expect(screen.getByText("42,180")).toBeInTheDocument();
  });

  it("reports a partial refresh as failed sources", async () => {
    vi.mocked(getTraySummary).mockResolvedValue(
      traySummaryResult({ ...summary, latestRefreshStatus: "partial" }),
    );

    renderTrayPanel();

    expect(await screen.findByText("Some sources failed")).toBeInTheDocument();
    expect(screen.getByText("42,180")).toBeInTheDocument();
  });

  it("gives a partial refresh precedence over estimated usage", async () => {
    vi.mocked(getTraySummary).mockResolvedValue(
      traySummaryResult({
        ...summary,
        dataQuality: "partial",
        latestRefreshStatus: "partial",
      }),
    );

    renderTrayPanel();

    expect(await screen.findByText("Some sources failed")).toBeInTheDocument();
    expect(
      screen.queryByText("Some usage is estimated"),
    ).not.toBeInTheDocument();
  });

  it("reports a failed latest refresh while keeping metrics visible", async () => {
    vi.mocked(getTraySummary).mockResolvedValue(
      traySummaryResult({ ...summary, latestRefreshStatus: "failed" }),
    );

    renderTrayPanel();

    expect(await screen.findByText("Refresh failed")).toBeInTheDocument();
    expect(screen.getByText("42,180")).toBeInTheDocument();
  });

  it("reports a cancelled latest refresh as failed", async () => {
    vi.mocked(getTraySummary).mockResolvedValue(
      traySummaryResult({ ...summary, latestRefreshStatus: "cancelled" }),
    );

    renderTrayPanel();

    expect(await screen.findByText("Refresh failed")).toBeInTheDocument();
  });

  it("shows failed-refresh copy while content stays empty", async () => {
    vi.mocked(getTraySummary).mockResolvedValue(
      traySummaryResult({
        ...summary,
        today: { ...summary.today, totalTokens: "0" },
        models: [],
        dataStatus: "empty",
        latestRefreshStatus: "failed",
      }),
    );

    renderTrayPanel();

    expect(await screen.findByText("Refresh failed")).toBeInTheDocument();
    expect(screen.getByText("No usage collected today")).toBeInTheDocument();
  });

  it("keeps the last summary visible while an active refresh runs", async () => {
    vi.mocked(getTraySummary)
      .mockResolvedValueOnce(traySummaryResult())
      .mockImplementation(
        () =>
          new Promise(() => {
            /* stays active */
          }),
      );

    const { rerender } = renderTrayPanel();
    expect(await screen.findByText("42,180")).toBeInTheDocument();

    rerender(
      <TrayPanel
        reportingTimezone="Europe/Berlin"
        appVersion="0.1.0"
        capabilities={capabilities}
      />,
    );

    expect(await screen.findByText("Refreshing")).toBeInTheDocument();
    expect(screen.getByText("42,180")).toBeInTheDocument();
  });
});
