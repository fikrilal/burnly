import { useTheme, isThemeChoice, type ThemeChoice } from "@/lib/theme";
import { MotionTabs } from "./motion-tabs";

const TABS: { id: ThemeChoice; label: string }[] = [
  { id: "light", label: "Light" },
  { id: "dark", label: "Dark" },
  { id: "system", label: "System" },
];

export function ThemeToggle({ className }: { className?: string }) {
  const { choice, setChoice } = useTheme();

  return (
    <MotionTabs
      tabs={TABS}
      activeTab={choice}
      onTabChange={(id) => {
        if (isThemeChoice(id)) {
          setChoice(id);
        }
      }}
      className={className}
      layoutId="theme-active-tab-indicator"
      hoverLayoutId="theme-hover-tab-indicator"
    />
  );
}
