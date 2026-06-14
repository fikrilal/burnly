import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { App } from "./App";

describe("App", () => {
  it("renders the Phase 0 Burnly shell", () => {
    render(<App />);

    expect(
      screen.getByRole("heading", { level: 1, name: "Burnly" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Phase 0")).toBeInTheDocument();
  });
});
