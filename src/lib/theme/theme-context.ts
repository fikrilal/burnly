import { createContext, useContext } from "react";

import type { ResolvedTheme, ThemeChoice } from "./theme";

export interface ThemeContextValue {
  choice: ThemeChoice;
  resolvedTheme: ResolvedTheme;
  setChoice: (choice: ThemeChoice) => void;
}

export const ThemeContext = createContext<ThemeContextValue | null>(null);

export function useTheme(): ThemeContextValue {
  const value = useContext(ThemeContext);
  if (value === null) {
    throw new Error("useTheme must be used within a ThemeProvider.");
  }
  return value;
}
