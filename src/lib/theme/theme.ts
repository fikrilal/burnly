export type ThemeChoice = "light" | "dark" | "system";
export type ResolvedTheme = "light" | "dark";

export const THEME_STORAGE_KEY = "burnly.theme";
export const DEFAULT_THEME_CHOICE: ThemeChoice = "dark";

export function isThemeChoice(value: unknown): value is ThemeChoice {
  return value === "light" || value === "dark" || value === "system";
}

export function resolveTheme(
  choice: ThemeChoice,
  systemPrefersDark: boolean,
): ResolvedTheme {
  if (choice === "system") {
    return systemPrefersDark ? "dark" : "light";
  }
  return choice;
}

export function readStoredChoice(
  storage: Pick<Storage, "getItem">,
): ThemeChoice {
  try {
    const raw = storage.getItem(THEME_STORAGE_KEY);
    return isThemeChoice(raw) ? raw : DEFAULT_THEME_CHOICE;
  } catch {
    return DEFAULT_THEME_CHOICE;
  }
}

export function storeChoice(
  storage: Pick<Storage, "setItem">,
  choice: ThemeChoice,
): void {
  try {
    storage.setItem(THEME_STORAGE_KEY, choice);
  } catch {
    // Persistence is best-effort; ignore storage failures.
  }
}

export function applyResolvedTheme(
  root: HTMLElement,
  resolved: ResolvedTheme,
): void {
  root.classList.toggle("dark", resolved === "dark");
}
