import type { ReactNode } from "react";
import { act, render, renderHook, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { ThemeProvider } from "./ThemeProvider";
import { useTheme } from "./theme-context";
import { THEME_STORAGE_KEY } from "./theme";

interface MatchMediaController {
  setMatches: (matches: boolean) => void;
}

function installMatchMedia(initialMatches: boolean): MatchMediaController {
  const listeners = new Set<EventListenerOrEventListenerObject>();
  let matches = initialMatches;

  window.matchMedia = (query: string): MediaQueryList => ({
    matches,
    media: query,
    onchange: null,
    addEventListener: (
      _type: string,
      listener: EventListenerOrEventListenerObject,
    ) => {
      listeners.add(listener);
    },
    removeEventListener: (
      _type: string,
      listener: EventListenerOrEventListenerObject,
    ) => {
      listeners.delete(listener);
    },
    addListener: () => undefined,
    removeListener: () => undefined,
    dispatchEvent: () => true,
  });

  return {
    setMatches(next: boolean) {
      matches = next;
      const event = Object.assign(new Event("change"), { matches: next });
      for (const listener of listeners) {
        if (typeof listener === "function") {
          listener(event);
        } else {
          listener.handleEvent(event);
        }
      }
    },
  };
}

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
