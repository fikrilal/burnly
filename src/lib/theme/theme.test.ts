import { describe, expect, it, vi } from "vitest";

import {
  DEFAULT_THEME_CHOICE,
  THEME_STORAGE_KEY,
  applyResolvedTheme,
  isThemeChoice,
  readStoredChoice,
  resolveTheme,
  storeChoice,
} from "./theme";

describe("resolveTheme", () => {
  it("returns the explicit choice for light and dark", () => {
    expect(resolveTheme("light", true)).toBe("light");
    expect(resolveTheme("dark", false)).toBe("dark");
  });

  it("follows the system preference when choice is system", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
  });
});

describe("isThemeChoice", () => {
  it("accepts valid choices and rejects everything else", () => {
    expect(isThemeChoice("light")).toBe(true);
    expect(isThemeChoice("dark")).toBe(true);
    expect(isThemeChoice("system")).toBe(true);
    expect(isThemeChoice("solar")).toBe(false);
    expect(isThemeChoice(null)).toBe(false);
  });
});

describe("readStoredChoice", () => {
  it("returns the stored choice when valid", () => {
    const storage = { getItem: vi.fn().mockReturnValue("light") };
    expect(readStoredChoice(storage)).toBe("light");
    expect(storage.getItem).toHaveBeenCalledWith(THEME_STORAGE_KEY);
  });

  it("falls back to the default when missing or invalid", () => {
    expect(readStoredChoice({ getItem: () => null })).toBe(
      DEFAULT_THEME_CHOICE,
    );
    expect(readStoredChoice({ getItem: () => "bogus" })).toBe(
      DEFAULT_THEME_CHOICE,
    );
  });

  it("falls back to the default when storage access throws", () => {
    expect(
      readStoredChoice({
        getItem: () => {
          throw new Error("blocked");
        },
      }),
    ).toBe(DEFAULT_THEME_CHOICE);
  });
});

describe("storeChoice", () => {
  it("writes the choice under the theme key", () => {
    const storage = { setItem: vi.fn() };
    storeChoice(storage, "system");
    expect(storage.setItem).toHaveBeenCalledWith(THEME_STORAGE_KEY, "system");
  });

  it("ignores storage failures", () => {
    expect(() => {
      storeChoice(
        {
          setItem: () => {
            throw new Error("blocked");
          },
        },
        "dark",
      );
    }).not.toThrow();
  });
});

describe("applyResolvedTheme", () => {
  it("toggles the dark class on the root element", () => {
    const root = document.createElement("div");
    applyResolvedTheme(root, "dark");
    expect(root.classList.contains("dark")).toBe(true);
    applyResolvedTheme(root, "light");
    expect(root.classList.contains("dark")).toBe(false);
  });
});
