import { useState } from "react";
import { motion } from "framer-motion";
import { cn } from "@/lib/cn";

interface Tab {
  id: string;
  label: string;
}

interface MotionTabsProps {
  tabs: Tab[];
  activeTab: string;
  onTabChange: (id: string) => void;
  className?: string | undefined;
  tabClassName?: string | undefined;
  activeTabClassName?: string | undefined;
  layoutId?: string | undefined;
  hoverLayoutId?: string | undefined;
}

const TAB_TRANSITION = {
  type: "spring",
  stiffness: 400,
  damping: 30,
} as const;

export function MotionTabs({
  tabs,
  activeTab,
  onTabChange,
  className,
  tabClassName,
  activeTabClassName,
  layoutId = "active-tab-indicator",
  hoverLayoutId = "hover-tab-indicator",
}: MotionTabsProps) {
  const [hoveredTab, setHoveredTab] = useState<string | null>(null);

  return (
    <div
      className={cn(
        "relative flex w-fit items-center justify-start rounded-lg bg-muted p-0.5",
        className,
      )}
      onMouseLeave={() => {
        setHoveredTab(null);
      }}
    >
      {tabs.map((tab) => (
        <MotionTabItem
          key={tab.id}
          tab={tab}
          isActive={activeTab === tab.id}
          isHovered={hoveredTab === tab.id}
          onClick={() => {
            onTabChange(tab.id);
          }}
          onMouseEnter={() => {
            setHoveredTab(tab.id);
          }}
          tabClassName={tabClassName}
          activeTabClassName={activeTabClassName}
          layoutId={layoutId}
          hoverLayoutId={hoverLayoutId}
        />
      ))}
    </div>
  );
}

function MotionTabItem({
  tab,
  isActive,
  isHovered,
  onClick,
  onMouseEnter,
  tabClassName,
  activeTabClassName,
  layoutId,
  hoverLayoutId,
}: {
  tab: Tab;
  isActive: boolean;
  isHovered: boolean;
  onClick: () => void;
  onMouseEnter: () => void;
  tabClassName?: string | undefined;
  activeTabClassName?: string | undefined;
  layoutId: string;
  hoverLayoutId: string;
}) {
  return (
    <button
      onClick={onClick}
      onMouseEnter={onMouseEnter}
      aria-pressed={isActive}
      className={cn(
        "relative z-10 flex-none px-2 py-0.5 font-medium transition-colors text-xs",
        isActive ? "text-foreground" : "text-muted-foreground",
        tabClassName,
        isActive && activeTabClassName,
      )}
      style={{
        WebkitTapHighlightColor: "transparent",
      }}
    >
      {isActive && (
        <motion.div
          layoutId={layoutId}
          className="absolute inset-0 z-0 rounded-md bg-background shadow-sm"
          transition={TAB_TRANSITION}
        />
      )}
      {!isActive && isHovered && (
        <motion.div
          layoutId={hoverLayoutId}
          className="absolute inset-0 z-0 rounded-md bg-muted-foreground/10"
          transition={TAB_TRANSITION}
        />
      )}
      <span className="relative z-10 text-xs leading-none">{tab.label}</span>
    </button>
  );
}
