import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import {
  EmptyState,
  ErrorState,
  FreshnessStatus,
  OpenDetailsButton,
} from "./status";

describe("FreshnessStatus", () => {
  it("renders a label for each state", () => {
    const { rerender } = render(<FreshnessStatus state="current" />);
    expect(screen.getByText("Current")).toBeInTheDocument();

    rerender(<FreshnessStatus state="refreshing" />);
    expect(screen.getByText("Refreshing")).toBeInTheDocument();

    rerender(<FreshnessStatus state="estimated" />);
    expect(screen.getByText("Some usage is estimated")).toBeInTheDocument();

    rerender(<FreshnessStatus state="partial" />);
    expect(screen.getByText("Some sources failed")).toBeInTheDocument();

    rerender(<FreshnessStatus state="failed" />);
    expect(screen.getByText("Refresh failed")).toBeInTheDocument();
  });
});

describe("EmptyState", () => {
  it("renders a title and description", () => {
    render(<EmptyState title="No usage today" description="It will update." />);
    expect(screen.getByText("No usage today")).toBeInTheDocument();
    expect(screen.getByText("It will update.")).toBeInTheDocument();
  });
});

describe("ErrorState", () => {
  it("renders an alert with a title", () => {
    render(<ErrorState title="Refresh failed" />);
    expect(screen.getByRole("alert")).toHaveTextContent("Refresh failed");
  });
});

describe("OpenDetailsButton", () => {
  it("renders its label and fires onClick", async () => {
    const user = userEvent.setup();
    const onClick = vi.fn();
    render(<OpenDetailsButton onClick={onClick} />);

    const button = screen.getByRole("button", { name: /open details/i });
    await user.click(button);
    expect(onClick).toHaveBeenCalledOnce();
  });
});
