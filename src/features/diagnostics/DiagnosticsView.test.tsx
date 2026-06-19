import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { getDiagnosticsStatus, type CommandResult } from "../../ipc/client";
import type { DiagnosticsStatusResponse } from "../../ipc/generated/contracts";
import { createTestQueryWrapper } from "../../test/query";
import { DiagnosticsView } from "./DiagnosticsView";

vi.mock("../../ipc/client");

const meta = {
  contractVersion: 1,
  requestId: "018f5f4d-7758-7bb2-9d9b-6d7f22c4a901",
  generatedAt: "2026-06-14T07:30:00.000Z",
} as const;

describe("DiagnosticsView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders loading state", () => {
    vi.mocked(getDiagnosticsStatus).mockImplementation(
      () =>
        new Promise((resolve) => {
          setTimeout(resolve, 100000);
        }),
    );

    render(<DiagnosticsView />, { wrapper: createTestQueryWrapper() });

    expect(screen.getByText("Loading diagnostics")).toBeInTheDocument();
  });

  it("renders component health summaries", async () => {
    vi.mocked(getDiagnosticsStatus).mockResolvedValue(
      diagnosticsResult(diagnosticsStatus()),
    );

    render(<DiagnosticsView />, { wrapper: createTestQueryWrapper() });

    expect(await screen.findByText("Runtime health")).toBeInTheDocument();
    expect(screen.getByText("Database is reachable.")).toBeInTheDocument();
    expect(screen.getByText("Sources are configured.")).toBeInTheDocument();
    expect(screen.getByText("Schema version 1")).toBeInTheDocument();
    expect(screen.getAllByText("healthy").length).toBeGreaterThan(0);
  });

  it("renders empty state when runtime returns no components", async () => {
    vi.mocked(getDiagnosticsStatus).mockResolvedValue(
      diagnosticsResult({ ...diagnosticsStatus(), components: [] }),
    );

    render(<DiagnosticsView />, { wrapper: createTestQueryWrapper() });

    expect(
      await screen.findByText("No diagnostics reported"),
    ).toBeInTheDocument();
  });

  it("renders error state and retries", async () => {
    const user = userEvent.setup();
    vi.mocked(getDiagnosticsStatus)
      .mockRejectedValueOnce(new Error("runtime unavailable"))
      .mockResolvedValueOnce(diagnosticsResult(diagnosticsStatus()));

    render(<DiagnosticsView />, { wrapper: createTestQueryWrapper() });

    expect(
      await screen.findByText("Diagnostics unavailable"),
    ).toBeInTheDocument();
    expect(screen.getByText("runtime unavailable")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Retry" }));

    await waitFor(() => {
      expect(screen.getByText("Runtime health")).toBeInTheDocument();
    });
  });
});

function diagnosticsResult(
  data: DiagnosticsStatusResponse,
): CommandResult<DiagnosticsStatusResponse> {
  return { data, meta };
}

function diagnosticsStatus(): DiagnosticsStatusResponse {
  return {
    status: "healthy",
    contractVersion: 1,
    components: [
      {
        component: "database",
        status: "healthy",
        summary: "Database is reachable.",
        details: ["Schema version 1"],
      },
      {
        component: "sources",
        status: "healthy",
        summary: "Sources are configured.",
        details: ["Detected sources 1", "Configured sources 1"],
      },
    ],
  };
}
