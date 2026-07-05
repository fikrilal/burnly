import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  copyDiagnosticsReport,
  exportDiagnosticsReport,
  getDiagnosticsHealth,
  getSettings,
  getTraySummary,
  updateSettings,
} from "../../ipc/client";
import { openExternalLink } from "../../ipc/external-links";
import {
  capabilities,
  diagnosticsHealthResult,
  renderTrayPanel,
  resetTrayPanelMocks,
  responseMeta,
  settingsResult,
  traySummaryResult,
} from "./test_support";

vi.mock("../../ipc/client");
vi.mock("../../ipc/external-links");
vi.mock("../../ipc/events");

beforeEach(resetTrayPanelMocks);

describe("TrayPanel close behavior settings", () => {
  it("renders persisted close behavior in settings", async () => {
    vi.mocked(getTraySummary).mockResolvedValue(traySummaryResult());
    vi.mocked(getSettings).mockResolvedValue(
      settingsResult({ closeBehavior: "quit" }),
    );

    renderTrayPanel();

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

    renderTrayPanel();

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

describe("TrayPanel launch at login settings", () => {
  it("updates launch at login when the runtime supports it", async () => {
    const user = userEvent.setup();
    vi.mocked(getTraySummary).mockResolvedValue(traySummaryResult());
    vi.mocked(getSettings).mockResolvedValue(
      settingsResult({
        launchAtLogin: false,
        closeBehavior: "hide",
        revision: 4,
      }),
    );
    vi.mocked(updateSettings).mockResolvedValue(
      settingsResult({
        launchAtLogin: true,
        closeBehavior: "hide",
        revision: 5,
      }),
    );

    renderTrayPanel();

    await user.click(await screen.findByRole("button", { name: "Settings" }));
    await user.click(
      await screen.findByRole("switch", { name: "Launch at login" }),
    );

    await waitFor(() => {
      expect(updateSettings).toHaveBeenCalledWith({
        launchAtLogin: true,
        closeBehavior: "hide",
        expectedRevision: 4,
      });
    });
  });

  it("disables launch at login when the runtime does not support it", async () => {
    const user = userEvent.setup();
    vi.mocked(getTraySummary).mockResolvedValue(traySummaryResult());
    vi.mocked(getSettings).mockResolvedValue(settingsResult());

    renderTrayPanel({
      capabilities: {
        ...capabilities,
        launchAtLogin: { supported: false, status: "not_implemented" },
      },
    });

    await user.click(await screen.findByRole("button", { name: "Settings" }));

    const launchAtLogin = await screen.findByRole("switch", {
      name: "Launch at login",
    });
    expect(launchAtLogin).toBeDisabled();
    await user.click(launchAtLogin);
    expect(updateSettings).not.toHaveBeenCalled();
  });
});

describe("TrayPanel settings failures", () => {
  it("renders settings load failures", async () => {
    const user = userEvent.setup();
    vi.mocked(getTraySummary).mockResolvedValue(traySummaryResult());
    vi.mocked(getSettings).mockRejectedValue(new Error("settings offline"));

    renderTrayPanel();

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

    renderTrayPanel();

    await user.click(await screen.findByRole("button", { name: "Settings" }));
    await user.click(
      await screen.findByRole("switch", { name: "Quit on close" }),
    );

    expect(await screen.findByText("Settings not saved")).toBeInTheDocument();
    expect(screen.getByText("settings conflict")).toBeInTheDocument();
  });
});

describe("TrayPanel diagnostics settings", () => {
  it("renders diagnostics health and local actions", async () => {
    const user = userEvent.setup();
    vi.mocked(getTraySummary).mockResolvedValue(traySummaryResult());
    vi.mocked(getSettings).mockResolvedValue(settingsResult());
    vi.mocked(getDiagnosticsHealth).mockResolvedValue(
      diagnosticsHealthResult({
        status: "warning",
        reasons: [
          {
            code: "diagnostics.sources_failed",
            message: "Some sources failed during the last refresh.",
          },
        ],
      }),
    );
    vi.mocked(exportDiagnosticsReport).mockResolvedValue({
      data: { status: "exported" },
      meta: responseMeta,
    });
    vi.mocked(copyDiagnosticsReport).mockResolvedValue({
      data: { status: "copied" },
      meta: responseMeta,
    });

    renderTrayPanel();

    await user.click(await screen.findByRole("button", { name: "Settings" }));

    expect(await screen.findByText("Diagnostics")).toBeInTheDocument();
    expect(
      await screen.findByLabelText("Diagnostics problem detected"),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Open diagnostics" }));
    expect(
      screen.getByText(
        /Burnly detected a problem. Export or copy diagnostics before reporting it./,
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "GitHub issue" })).toHaveAttribute(
      "href",
      "https://github.com/fikrilal/burnly/issues",
    );
    await user.click(screen.getByRole("link", { name: "GitHub issue" }));
    expect(openExternalLink).toHaveBeenCalledWith(
      "https://github.com/fikrilal/burnly/issues",
    );
    await user.click(screen.getByRole("button", { name: "Export" }));
    expect(exportDiagnosticsReport).toHaveBeenCalled();
    expect(
      await screen.findByText("Diagnostics report exported."),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Copy" }));
    expect(copyDiagnosticsReport).toHaveBeenCalled();
    expect(
      await screen.findByText("Diagnostics report copied."),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
  });
});
