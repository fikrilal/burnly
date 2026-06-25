import type { ReactNode } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { ThemeToggle } from "@/components/ui/theme-toggle";
import { CompactCard, StatusPill } from "@/components/burnly";

const SURFACE_TOKENS = [
  { name: "background", className: "bg-background" },
  { name: "card", className: "bg-card" },
  { name: "muted", className: "bg-muted" },
  { name: "primary", className: "bg-primary" },
  { name: "secondary", className: "bg-secondary" },
  { name: "accent", className: "bg-accent" },
  { name: "destructive", className: "bg-destructive" },
] as const;

export function StyleguideView() {
  return (
    <main className="min-h-screen bg-background text-foreground">
      <div className="mx-auto w-full max-w-4xl px-6 py-10">
        <header className="flex items-center justify-between gap-4">
          <div>
            <h1 className="text-2xl font-semibold tracking-tight">
              Burnly Design System
            </h1>
            <p className="mt-1 text-sm text-muted-foreground">
              Token-based primitives, rendered in the active theme.
            </p>
          </div>
          <ThemeToggle />
        </header>

        <Separator className="my-8" />

        <div className="flex flex-col gap-10">
          <SurfaceTokens />
          <Typography />
          <Buttons />
          <Badges />
          <StatusPills />
          <SkeletonSample />
          <CardSample />
        </div>
      </div>
    </main>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h2 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
        {title}
      </h2>
      <div className="mt-3">{children}</div>
    </section>
  );
}

function SurfaceTokens() {
  return (
    <Section title="Surfaces">
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        {SURFACE_TOKENS.map((token) => (
          <div key={token.name} className="flex flex-col gap-2">
            <div
              className={`h-14 rounded-lg border border-border ${token.className}`}
            />
            <span className="text-xs text-muted-foreground">{token.name}</span>
          </div>
        ))}
      </div>
    </Section>
  );
}

function Typography() {
  return (
    <Section title="Typography">
      <div className="space-y-2">
        <p className="text-3xl font-semibold tracking-tight">Display 3xl</p>
        <p className="text-xl font-semibold">Heading xl</p>
        <p className="text-base text-foreground">Body base</p>
        <p className="text-sm text-muted-foreground">Muted sm</p>
        <p className="text-xs text-muted-foreground">Caption xs</p>
      </div>
    </Section>
  );
}

function Buttons() {
  return (
    <Section title="Buttons">
      <div className="flex flex-wrap items-center gap-3">
        <Button>Default</Button>
        <Button variant="secondary">Secondary</Button>
        <Button variant="outline">Outline</Button>
        <Button variant="ghost">Ghost</Button>
        <Button variant="destructive">Destructive</Button>
        <Button variant="link">Link</Button>
      </div>
    </Section>
  );
}

function Badges() {
  return (
    <Section title="Badges">
      <div className="flex flex-wrap items-center gap-3">
        <Badge>Default</Badge>
        <Badge variant="secondary">Secondary</Badge>
        <Badge variant="outline">Outline</Badge>
        <Badge variant="destructive">Destructive</Badge>
      </div>
    </Section>
  );
}

function StatusPills() {
  return (
    <Section title="Status pills">
      <div className="flex flex-wrap items-center gap-3">
        <StatusPill tone="neutral">Neutral</StatusPill>
        <StatusPill tone="success">Current</StatusPill>
        <StatusPill tone="warning">Stale</StatusPill>
        <StatusPill tone="danger">Failed</StatusPill>
      </div>
    </Section>
  );
}

function SkeletonSample() {
  return (
    <Section title="Skeleton">
      <div className="space-y-2">
        <Skeleton className="h-4 w-48" />
        <Skeleton className="h-4 w-32" />
        <Skeleton className="h-10 w-full" />
      </div>
    </Section>
  );
}

function CardSample() {
  return (
    <Section title="Compact card">
      <CompactCard className="max-w-sm p-5">
        <p className="text-sm text-muted-foreground">Today token usage</p>
        <p className="mt-2 text-3xl font-semibold tracking-tight">42,180</p>
        <p className="mt-1 text-xs text-muted-foreground">tokens today</p>
      </CompactCard>
    </Section>
  );
}
