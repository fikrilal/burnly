import type { ReactNode } from "react";
import { act, render, renderHook, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { ThemeProvider } from "./ThemeProvider";
import { useTheme } from "./theme-context";
import { THEME_STORAGE_KEY } from "./theme";
import { installMatchMedia } from "@/test/match-media";

function Wrapper({ children }: { children: ReactNode }) {
  return <ThemeProvider>{children}</ThemeProvider>;
}

function ThemeProbe() {
  const { choice } = useTheme();
  return <span data-testid="choice">{choice}</span>;
}

beforeEach(() => {
  window.localStorage.clear();
  document.documentElement.classList.remove("dark");
});

afterEach(() => {
  window.localStorage.clear();
});

describe("ThemeProvider", () => {
  it("applies the default dark theme when nothing is stored", () => {
    installMatchMedia(false);
    const { result } = renderHook(() => useTheme(), { wrapper: Wrapper });

    expect(result.current.choice).toBe("dark");
    expect(result.current.resolvedTheme).toBe("dark");
    expect(document.documentElement.classList.contains("dark")).toBe(true);
  });

  it("persists the choice and updates the resolved theme", () => {
    installMatchMedia(false);
    const { result } = renderHook(() => useTheme(), { wrapper: Wrapper });

    act(() => {
      result.current.setChoice("light");
    });

    expect(result.current.resolvedTheme).toBe("light");
    expect(document.documentElement.classList.contains("dark")).toBe(false);
    expect(window.localStorage.getItem(THEME_STORAGE_KEY)).toBe("light");
  });

  it("follows the system preference and reacts to changes", () => {
    const controller = installMatchMedia(true);
    window.localStorage.setItem(THEME_STORAGE_KEY, "system");
    const { result } = renderHook(() => useTheme(), { wrapper: Wrapper });

    expect(result.current.resolvedTheme).toBe("dark");

    act(() => {
      controller.setMatches(false);
    });

    expect(result.current.resolvedTheme).toBe("light");
    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });

  it("restores a stored choice on mount", () => {
    installMatchMedia(false);
    window.localStorage.setItem(THEME_STORAGE_KEY, "light");

    render(
      <ThemeProvider>
        <ThemeProbe />
      </ThemeProvider>,
    );

    expect(screen.getByTestId("choice").textContent).toBe("light");
  });
});

describe("useTheme", () => {
  it("throws when used outside a provider", () => {
    expect(() => renderHook(() => useTheme())).toThrow(
      /must be used within a ThemeProvider/,
    );
  });
});
