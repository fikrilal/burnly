import { screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { getTraySummary } from "../../ipc/client";
import {
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
});
