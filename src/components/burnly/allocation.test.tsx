import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { AllocationList, TrendIndicator } from "./allocation";

describe("TrendIndicator", () => {
  it("renders 'new today' when there is no trend", () => {
    render(<TrendIndicator trend={null} />);
    expect(screen.getByText("new today")).toBeInTheDocument();
  });

  it("renders a percentage for an increased trend", () => {
    render(
      <TrendIndicator trend={{ direction: "increased", basisPoints: 850 }} />,
    );
    expect(screen.getByText("8.5%")).toBeInTheDocument();
  });

  it("renders whole percentages without decimals", () => {
    render(<TrendIndicator trend={{ direction: "flat", basisPoints: 0 }} />);
    expect(screen.getByText("0%")).toBeInTheDocument();
  });
});

describe("AllocationList", () => {
  it("renders each model's name, agent, and tokens", () => {
    render(
      <AllocationList
        models={[
          {
            modelName: "GPT-5.1",
            agentLabel: "Codex",
            tokens: "25,000",
            trend: { direction: "increased", basisPoints: 850 },
          },
          {
            modelName: "Claude Sonnet",
            agentLabel: "Claude Code",
            tokens: "12,000",
            trend: null,
          },
        ]}
      />,
    );
    expect(screen.getByText("GPT-5.1")).toBeInTheDocument();
    expect(screen.getByText("Codex")).toBeInTheDocument();
    expect(screen.getByText("25,000")).toBeInTheDocument();
    expect(screen.getByText("Claude Sonnet")).toBeInTheDocument();
    expect(screen.getByText("new today")).toBeInTheDocument();
  });

  it("renders an empty state when there are no models", () => {
    render(<AllocationList models={[]} emptyLabel="No model usage today" />);
    expect(screen.getByText("No model usage today")).toBeInTheDocument();
  });
});
