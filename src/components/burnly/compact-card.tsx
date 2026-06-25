import type { ReactNode } from "react";

interface CompactCardProps {
  children: ReactNode;
  className?: string;
}

export function CompactCard({ children, className = "" }: CompactCardProps) {
  return (
    <section
      className={`rounded-2xl border border-border bg-card text-card-foreground shadow-xl shadow-black/20 ${className}`}
    >
      {children}
    </section>
  );
}

interface StatusPillProps {
  children: ReactNode;
  tone?: "neutral" | "success" | "warning" | "danger";
}

export function StatusPill({ children, tone = "neutral" }: StatusPillProps) {
  return (
    <span
      className={`inline-flex items-center rounded-full px-2.5 py-1 text-xs font-medium ${toneClass(tone)}`}
    >
      {children}
    </span>
  );
}

function toneClass(tone: NonNullable<StatusPillProps["tone"]>): string {
  switch (tone) {
    // Monochrome design: success/warning share a subtle emphasis treatment;
    // color is reserved for the destructive (error) state.
    case "success":
    case "warning":
      return "bg-foreground/10 text-foreground ring-1 ring-border";
    case "danger":
      return "bg-destructive/10 text-destructive ring-1 ring-destructive/20";
    case "neutral":
      return "bg-muted text-muted-foreground ring-1 ring-border";
  }
}
