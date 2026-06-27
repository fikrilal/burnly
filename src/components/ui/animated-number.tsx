import { useEffect, useRef, useState } from "react";
import { animate, useReducedMotion } from "motion/react";

import { cn } from "@/lib/cn";

// Adapted from beUI's AnimatedNumber (https://beui.dev/components/motion/number, MIT).
// Animates from the previous value to the next on change; jumps instantly when
// the user prefers reduced motion. No viewport gating — Burnly surfaces that use
// this (the tray panel) are visible as soon as they mount.
const EASE_OUT = [0.16, 1, 0.3, 1] as const;

export interface AnimatedNumberProps {
  value: number;
  duration?: number;
  format?: (value: number) => string;
  className?: string;
}

export function AnimatedNumber({
  value,
  duration = 1,
  format = (n) => Math.round(n).toLocaleString(),
  className,
}: AnimatedNumberProps) {
  const reduce = useReducedMotion();
  const [display, setDisplay] = useState(value);
  const fromRef = useRef(value);

  useEffect(() => {
    if (reduce) {
      fromRef.current = value;
      return;
    }

    const controls = animate(fromRef.current, value, {
      duration,
      ease: EASE_OUT,
      onUpdate: (current) => {
        setDisplay(current);
      },
    });
    fromRef.current = value;

    return () => {
      controls.stop();
    };
  }, [value, duration, reduce]);

  const shown = reduce ? value : display;

  return (
    <span data-slot="animated-number" className={cn("tabular-nums", className)}>
      {format(shown)}
    </span>
  );
}
