import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  checkForUpdate,
  downloadUpdate,
  getSettings,
  getTraySummary,
  getUpdateState,
  restartForUpdate,
} from "../../ipc/client";
import {
  renderTrayPanel,
  resetTrayPanelMocks,
  settingsResult,
  traySummaryResult,
  updateResult,
} from "./test_support";

vi.mock("../../ipc/client");
vi.mock("../../ipc/external-links");
vi.mock("../../ipc/events");

beforeEach(resetTrayPanelMocks);

describe("TrayPanel update check and install actions", () => {
  it("renders updater state and triggers check action", async () => {
    const user = userEvent.setup();
    vi.mocked(getTraySummary).mockResolvedValue(traySummaryResult());
    vi.mocked(getSettings).mockResolvedValue(settingsResult());
    vi.mocked(getUpdateState).mockResolvedValue(
      updateResult({ status: "idle" }),
    );
    vi.mocked(checkForUpdate).mockResolvedValue(
      updateResult({ status: "available", availableVersion: "1.2.0" }),
    );

    renderTrayPanel();

    await user.click(await screen.findByRole("button", { name: "Settings" }));

    expect(await screen.findByText("Updates")).toBeInTheDocument();
    expect(
      screen.getByText("Check for updates to get the latest features."),
    ).toBeInTheDocument();

    const checkButton = screen.getByRole("button", { name: "Check" });
    await user.click(checkButton);

    expect(checkForUpdate).toHaveBeenCalled();
    expect(
      await screen.findByText("Version 1.2.0 is available."),
    ).toBeInTheDocument();
  });

  it("renders available update and triggers install action", async () => {
    const user = userEvent.setup();
    vi.mocked(getTraySummary).mockResolvedValue(traySummaryResult());
    vi.mocked(getSettings).mockResolvedValue(settingsResult());
    vi.mocked(getUpdateState).mockResolvedValue(
      updateResult({ status: "available", availableVersion: "1.2.0" }),
    );
    vi.mocked(downloadUpdate).mockResolvedValue(
      updateResult({ status: "ready", downloadedVersion: "1.2.0" }),
    );

    renderTrayPanel();

    await user.click(await screen.findByRole("button", { name: "Settings" }));

    expect(
      await screen.findByText("Version 1.2.0 is available."),
    ).toBeInTheDocument();

    const installButton = screen.getByRole("button", { name: "Install" });
    await user.click(installButton);

    expect(downloadUpdate).toHaveBeenCalled();
    expect(
      await screen.findByText("Version 1.2.0 is ready."),
    ).toBeInTheDocument();
  });
});

describe("TrayPanel update restart action", () => {
  it("renders ready update and triggers restart action", async () => {
    const user = userEvent.setup();
    vi.mocked(getTraySummary).mockResolvedValue(traySummaryResult());
    vi.mocked(getSettings).mockResolvedValue(settingsResult());
    vi.mocked(getUpdateState).mockResolvedValue(
      updateResult({ status: "ready", downloadedVersion: "1.2.0" }),
    );
    vi.mocked(restartForUpdate).mockResolvedValue(
      updateResult({ status: "idle" }),
    );

    renderTrayPanel();

    await user.click(await screen.findByRole("button", { name: "Settings" }));

    expect(
      await screen.findByText("Version 1.2.0 is ready."),
    ).toBeInTheDocument();

    const restartButton = screen.getByRole("button", { name: "Restart" });
    await user.click(restartButton);

    expect(restartForUpdate).toHaveBeenCalled();
  });
});

describe("TrayPanel update unavailable states", () => {
  it("renders unavailable updater quietly as disabled row", async () => {
    const user = userEvent.setup();
    vi.mocked(getTraySummary).mockResolvedValue(traySummaryResult());
    vi.mocked(getSettings).mockResolvedValue(settingsResult());
    vi.mocked(getUpdateState).mockResolvedValue(
      updateResult({ status: "unavailable" }),
    );

    renderTrayPanel();

    await user.click(await screen.findByRole("button", { name: "Settings" }));

    expect(
      await screen.findByText("Updates are not available for this build."),
    ).toBeInTheDocument();
    const checkButton = screen.getByRole("button", { name: "Check" });
    expect(checkButton).toBeDisabled();
  });

  it("renders update state load failures instead of a permanent loading row", async () => {
    const user = userEvent.setup();
    vi.mocked(getTraySummary).mockResolvedValue(traySummaryResult());
    vi.mocked(getSettings).mockResolvedValue(settingsResult());
    vi.mocked(getUpdateState).mockRejectedValue(
      new Error("update status unavailable"),
    );

    renderTrayPanel();

    await user.click(await screen.findByRole("button", { name: "Settings" }));

    expect(
      await screen.findByText("Burnly could not load update status."),
    ).toBeInTheDocument();
    expect(screen.getByText("update status unavailable")).toBeInTheDocument();
    expect(
      screen.queryByText("Checking update status..."),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Check" })).toBeDisabled();
  });
});

describe("TrayPanel update failures", () => {
  it("does not offer check action for non-retryable failed update states", async () => {
    const user = userEvent.setup();
    vi.mocked(getTraySummary).mockResolvedValue(traySummaryResult());
    vi.mocked(getSettings).mockResolvedValue(settingsResult());
    vi.mocked(getUpdateState).mockResolvedValue(
      updateResult({
        status: "failed",
        error: { code: "update.signature_failed", retryable: false },
      }),
    );

    renderTrayPanel();

    await user.click(await screen.findByRole("button", { name: "Settings" }));

    expect(
      await screen.findByText(
        "Burnly cannot continue this update automatically.",
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Burnly could not verify the update signature."),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Check" })).toBeDisabled();
    expect(checkForUpdate).not.toHaveBeenCalled();
  });

  it("renders update command errors using user-safe copy", async () => {
    const user = userEvent.setup();
    vi.mocked(getTraySummary).mockResolvedValue(traySummaryResult());
    vi.mocked(getSettings).mockResolvedValue(settingsResult());
    vi.mocked(getUpdateState).mockResolvedValue(
      updateResult({ status: "idle" }),
    );
    vi.mocked(checkForUpdate).mockRejectedValue(
      new Error("Update service offline"),
    );

    renderTrayPanel();

    await user.click(await screen.findByRole("button", { name: "Settings" }));

    const checkButton = screen.getByRole("button", { name: "Check" });
    await user.click(checkButton);

    expect(await screen.findByText("Update failed")).toBeInTheDocument();
    expect(screen.getByText("Update service offline")).toBeInTheDocument();
  });
});
