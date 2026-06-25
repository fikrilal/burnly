import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";

import { ThemeProvider } from "@/lib/theme";
import { installMatchMedia } from "@/test/match-media";
import { ThemeToggle } from "./theme-toggle";

beforeEach(() => {
  window.localStorage.clear();
  document.documentElement.classList.remove("dark");
  installMatchMedia(false);
});

function renderToggle() {
  return render(
    <ThemeProvider>
      <ThemeToggle />
    </ThemeProvider>,
  );
}

describe("ThemeToggle", () => {
  it("renders the three theme options", () => {
    renderToggle();
    expect(screen.getByRole("button", { name: /light/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /dark/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /system/i })).toBeInTheDocument();
  });

  it("marks the active choice as pressed (default dark)", () => {
    renderToggle();
    expect(screen.getByRole("button", { name: /dark/i })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("button", { name: /light/i })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
  });

  it("switches the resolved theme when an option is selected", async () => {
    const user = userEvent.setup();
    renderToggle();

    await user.click(screen.getByRole("button", { name: /light/i }));
    expect(document.documentElement.classList.contains("dark")).toBe(false);
    expect(screen.getByRole("button", { name: /light/i })).toHaveAttribute(
      "aria-pressed",
      "true",
    );

    await user.click(screen.getByRole("button", { name: /dark/i }));
    expect(document.documentElement.classList.contains("dark")).toBe(true);
  });
});
