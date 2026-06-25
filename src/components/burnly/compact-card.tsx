import type { ReactNode } from "react";

interface CompactCardProps {
  children: ReactNode;
  className?: string;
}

export function CompactCard({ children, className = "" }: CompactCardProps) {
  return (
    <section
      className={`rounded-2xl border border-zinc-800 bg-zinc-900/80 shadow-xl shadow-black/20 ${className}`}
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
    case "success":
      return "bg-emerald-500/10 text-emerald-300 ring-1 ring-emerald-500/20";
    case "warning":
      return "bg-amber-500/10 text-amber-300 ring-1 ring-amber-500/20";
    case "danger":
      return "bg-red-500/10 text-red-300 ring-1 ring-red-500/20";
    case "neutral":
      return "bg-zinc-800 text-zinc-300 ring-1 ring-zinc-700";
  }
}
