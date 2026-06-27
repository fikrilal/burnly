import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

import { ThemeContext } from "./theme-context";
import {
  applyResolvedTheme,
  readStoredChoice,
  resolveTheme,
  storeChoice,
  type ThemeChoice,
} from "./theme";

const SYSTEM_DARK_QUERY = "(prefers-color-scheme: dark)";

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [choice, setChoiceState] = useState<ThemeChoice>(() =>
    readStoredChoice(window.localStorage),
  );
  const [systemPrefersDark, setSystemPrefersDark] = useState<boolean>(
    () => window.matchMedia(SYSTEM_DARK_QUERY).matches,
  );

  useEffect(() => {
    const query = window.matchMedia(SYSTEM_DARK_QUERY);
    const onChange = (event: MediaQueryListEvent) => {
      setSystemPrefersDark(event.matches);
    };
    query.addEventListener("change", onChange);
    return () => {
      query.removeEventListener("change", onChange);
    };
  }, []);

  const resolvedTheme = resolveTheme(choice, systemPrefersDark);

  useEffect(() => {
    applyResolvedTheme(document.documentElement, resolvedTheme);
  }, [resolvedTheme]);

  const setChoice = useCallback((next: ThemeChoice) => {
    setChoiceState(next);
    storeChoice(window.localStorage, next);
  }, []);

  const value = useMemo(
    () => ({ choice, resolvedTheme, setChoice }),
    [choice, resolvedTheme, setChoice],
  );

  return <ThemeContext value={value}>{children}</ThemeContext>;
}
