import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";

import { ThemeProvider } from "@/lib/theme";
import { installMatchMedia } from "@/test/match-media";
import { StyleguideView } from "./StyleguideView";

beforeEach(() => {
  window.localStorage.clear();
  document.documentElement.classList.remove("dark");
  installMatchMedia(false);
});

function renderStyleguide() {
  return render(
    <ThemeProvider>
      <StyleguideView />
    </ThemeProvider>,
  );
}

describe("StyleguideView", () => {
  it("renders the design system heading and key sections", () => {
    renderStyleguide();
    expect(
      screen.getByRole("heading", { name: /burnly design system/i }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("heading", { name: /^surfaces$/i }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("heading", { name: /^buttons$/i }),
    ).toBeInTheDocument();
  });

  it("includes a working theme toggle", async () => {
    const user = userEvent.setup();
    renderStyleguide();

    await user.click(screen.getByRole("button", { name: /light/i }));
    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });
});
