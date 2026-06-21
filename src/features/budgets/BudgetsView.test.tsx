import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  createBudget,
  deleteBudget,
  disableBudget,
  enableBudget,
  listBudgets,
  updateBudget,
} from "../../ipc/client";
import { BurnlyClientError } from "../../ipc/errors";
import type { BudgetResponse } from "../../ipc/generated/contracts";
import { BudgetsView } from "./BudgetsView";

vi.mock("../../ipc/client", () => ({
  createBudget: vi.fn(),
  deleteBudget: vi.fn(),
  disableBudget: vi.fn(),
  enableBudget: vi.fn(),
  listBudgets: vi.fn(),
  updateBudget: vi.fn(),
}));

vi.mock("../../ipc/events", () => ({
  EVENT_NAMES: { dataInvalidated: "burnly://v1/data-invalidated" },
  subscribeToEvent: vi.fn().mockResolvedValue(() => undefined),
}));

const meta = {
  contractVersion: 1,
  requestId: "request-1",
  generatedAt: "2026-06-18T00:00:00.000Z",
};

describe("BudgetsView", () => {
  beforeEach(() => {
    setupMocks();
  });

  it("creates a token budget through the IPC boundary", async () => {
    const user = userEvent.setup();
    render(<BudgetsView />, { wrapper: queryWrapper() });

    await user.type(await screen.findByLabelText("Name"), "Monthly token cap");
    await user.clear(screen.getByLabelText("Token limit"));
    await user.type(screen.getByLabelText("Token limit"), "2500000");
    await user.click(screen.getByRole("button", { name: "Create budget" }));

    expect(createBudget).toHaveBeenCalledWith({
      budget: {
        name: "Monthly token cap",
        limit: { kind: "tokens", value: "2500000" },
        period: "monthly",
        scope: { kind: "global" },
        enabled: true,
        thresholds: [
          { basisPoints: 8000, enabled: true },
          { basisPoints: 10000, enabled: true },
        ],
      },
    });
    expect(await screen.findByText("Budget saved.")).toBeInTheDocument();
  });

  it("edits a source-scoped cost budget without sending token fields", async () => {
    const user = userEvent.setup();
    vi.mocked(listBudgets).mockResolvedValue({
      data: { items: [costBudget()] },
      meta,
    });

    render(<BudgetsView />, { wrapper: queryWrapper() });

    await user.click(await screen.findByRole("button", { name: "Edit" }));
    await user.clear(screen.getByLabelText("Cost limit"));
    await user.type(screen.getByLabelText("Cost limit"), "25.50");
    await user.click(screen.getByRole("button", { name: "Save budget" }));

    expect(updateBudget).toHaveBeenCalledWith({
      budgetId: "8",
      expectedRevision: "3",
      budget: {
        name: "Source cost cap",
        limit: {
          kind: "cost",
          amountMicros: "25500000",
          currency: "USD",
        },
        period: "weekly",
        scope: { kind: "source", sourceId: "2" },
        enabled: true,
        thresholds: [{ basisPoints: 9000, enabled: true }],
      },
    });
  });

  it("prevents invalid duplicate thresholds before transport", async () => {
    const user = userEvent.setup();
    render(<BudgetsView />, { wrapper: queryWrapper() });

    await user.type(await screen.findByLabelText("Name"), "Duplicate warning");
    await user.clear(screen.getByLabelText("Threshold 1"));
    await user.type(screen.getByLabelText("Threshold 1"), "90");
    await user.clear(screen.getByLabelText("Threshold 2"));
    await user.type(screen.getByLabelText("Threshold 2"), "90");
    await user.click(screen.getByRole("button", { name: "Create budget" }));

    expect(createBudget).not.toHaveBeenCalled();
    expect(
      screen.getByText("Threshold percentages must be unique."),
    ).toBeInTheDocument();
  });

  it("requires confirmation before deleting a budget", async () => {
    const user = userEvent.setup();
    vi.mocked(listBudgets).mockResolvedValue({
      data: { items: [tokenBudget()] },
      meta,
    });

    render(<BudgetsView />, { wrapper: queryWrapper() });

    await user.click(await screen.findByRole("button", { name: "Delete" }));
    expect(screen.getByText("Delete budget?")).toBeInTheDocument();
    expect(deleteBudget).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Delete budget" }));
    expect(deleteBudget).toHaveBeenCalledWith({
      budgetId: "7",
      expectedRevision: "1",
    });
  });

  it("preserves form input on revision conflict and offers reload", async () => {
    const user = userEvent.setup();
    vi.mocked(listBudgets).mockResolvedValue({
      data: { items: [tokenBudget()] },
      meta,
    });
    vi.mocked(updateBudget).mockRejectedValue(
      new BurnlyClientError({
        kind: "application",
        error: {
          code: "budgets.revision_conflict",
          message: "Budget changed elsewhere.",
          category: "conflict",
          retryable: true,
          details: null,
        },
        requestId: meta.requestId,
        generatedAt: meta.generatedAt,
      }),
    );

    render(<BudgetsView />, { wrapper: queryWrapper() });

    await user.click(await screen.findByRole("button", { name: "Edit" }));
    const name = screen.getByLabelText("Name");
    await user.clear(name);
    await user.type(name, "Edited local draft");
    await user.click(screen.getByRole("button", { name: "Save budget" }));

    expect(
      await screen.findByText("Budget changed elsewhere."),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Reload budgets" }),
    ).toBeInTheDocument();
    expect(screen.getByDisplayValue("Edited local draft")).toBeInTheDocument();
  });

  it("enables existing budgets with revisions", async () => {
    const user = userEvent.setup();
    vi.mocked(listBudgets).mockResolvedValue({
      data: { items: [{ ...tokenBudget(), enabled: false }] },
      meta,
    });

    render(<BudgetsView />, { wrapper: queryWrapper() });

    await user.click(await screen.findByRole("button", { name: "Enable" }));
    expect(enableBudget).toHaveBeenCalledWith({
      budgetId: "7",
      expectedRevision: "1",
    });
  });

  it("disables existing budgets with revisions", async () => {
    const user = userEvent.setup();

    vi.mocked(listBudgets).mockResolvedValue({
      data: { items: [tokenBudget()] },
      meta,
    });
    render(<BudgetsView />, { wrapper: queryWrapper() });

    await user.click(await screen.findByRole("button", { name: "Disable" }));
    expect(disableBudget).toHaveBeenCalledWith({
      budgetId: "7",
      expectedRevision: "1",
    });
  });
});

function setupMocks() {
  vi.clearAllMocks();
  vi.mocked(listBudgets).mockResolvedValue({ data: { items: [] }, meta });
  vi.mocked(createBudget).mockResolvedValue({
    data: tokenBudget({ name: "Monthly token cap", revision: "2" }),
    meta,
  });
  vi.mocked(updateBudget).mockResolvedValue({
    data: costBudget({
      limit: { kind: "cost", amountMicros: "25500000", currency: "USD" },
      revision: "4",
    }),
    meta,
  });
  vi.mocked(enableBudget).mockResolvedValue({
    data: tokenBudget({ enabled: true, revision: "2" }),
    meta,
  });
  vi.mocked(disableBudget).mockResolvedValue({
    data: tokenBudget({ enabled: false, revision: "2" }),
    meta,
  });
  vi.mocked(deleteBudget).mockResolvedValue({
    data: { budgetId: "7" },
    meta,
  });
}

function tokenBudget(overrides: Partial<BudgetResponse> = {}): BudgetResponse {
  return {
    id: "7",
    revision: "1",
    name: "Token cap",
    limit: { kind: "tokens", value: "1000000" },
    period: "monthly",
    scope: { kind: "global" },
    enabled: true,
    thresholds: [
      { basisPoints: 8000, enabled: true },
      { basisPoints: 10000, enabled: true },
    ],
    ...overrides,
  };
}

function costBudget(overrides: Partial<BudgetResponse> = {}): BudgetResponse {
  return {
    id: "8",
    revision: "3",
    name: "Source cost cap",
    limit: { kind: "cost", amountMicros: "12500000", currency: "USD" },
    period: "weekly",
    scope: { kind: "source", sourceId: "2" },
    enabled: true,
    thresholds: [{ basisPoints: 9000, enabled: true }],
    ...overrides,
  };
}

function queryWrapper() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
  };
}
