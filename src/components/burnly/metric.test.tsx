import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { CompactMetric, MetricRow } from "./metric";

describe("CompactMetric", () => {
  it("renders label, value, and caption", () => {
    render(
      <CompactMetric label="Today" value="42,180" caption="tokens today" />,
    );
    expect(screen.getByText("Today")).toBeInTheDocument();
    expect(screen.getByText("42,180")).toBeInTheDocument();
    expect(screen.getByText("tokens today")).toBeInTheDocument();
  });

  it("renders without a caption", () => {
    render(<CompactMetric label="Today" value="42,180" />);
    expect(screen.getByText("42,180")).toBeInTheDocument();
  });
});

describe("MetricRow", () => {
  it("renders each item's label and value", () => {
    render(
      <MetricRow
        items={[
          { label: "This week", value: "183,240" },
          { label: "This month", value: "612,900" },
        ]}
      />,
    );
    expect(screen.getByText("This week")).toBeInTheDocument();
    expect(screen.getByText("183,240")).toBeInTheDocument();
    expect(screen.getByText("This month")).toBeInTheDocument();
    expect(screen.getByText("612,900")).toBeInTheDocument();
  });
});
