import type { ReactNode } from "react";
import { useState } from "react";

import { AnimatedNumber } from "@/components/ui/animated-number";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { ThemeToggle } from "@/components/ui/theme-toggle";
import {
  CompactCard,
  CompactMetric,
  MetricRow,
  StatusPill,
} from "@/components/burnly";

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
          <MetricsSample />
          <Buttons />
          <Badges />
          <StatusPills />
          <TabsSample />
          <SwitchSample />
          <NumbersSample />
          <Overlays />
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

function Overlays() {
  return (
    <Section title="Overlays">
      <div className="flex flex-wrap items-center gap-3">
        <TooltipProvider delayDuration={0}>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="outline">Tooltip</Button>
            </TooltipTrigger>
            <TooltipContent>Updated 2m ago</TooltipContent>
          </Tooltip>
        </TooltipProvider>

        <Popover>
          <PopoverTrigger asChild>
            <Button variant="outline">Popover</Button>
          </PopoverTrigger>
          <PopoverContent>
            <p className="text-sm text-muted-foreground">
              Popover content on the popover surface token.
            </p>
          </PopoverContent>
        </Popover>

        <Dialog>
          <DialogTrigger asChild>
            <Button variant="outline">Dialog</Button>
          </DialogTrigger>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>Delete history</DialogTitle>
              <DialogDescription>
                This permanently removes stored usage history.
              </DialogDescription>
            </DialogHeader>
            <DialogFooter>
              <DialogClose asChild>
                <Button variant="ghost">Cancel</Button>
              </DialogClose>
              <Button variant="destructive">Delete</Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>

        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="outline">Menu</Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent>
            <DropdownMenuLabel>Actions</DropdownMenuLabel>
            <DropdownMenuItem>Refresh now</DropdownMenuItem>
            <DropdownMenuItem>Open diagnostics</DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem>Export</DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </Section>
  );
}

function MetricsSample() {
  return (
    <Section title="Metrics">
      <div className="max-w-sm space-y-5">
        <CompactMetric
          label="Today token usage"
          value={<AnimatedNumber value={42180} />}
          caption="tokens today"
        />
        <MetricRow
          items={[
            { label: "This week", value: "183,240" },
            { label: "This month", value: "612,900" },
          ]}
        />
      </div>
    </Section>
  );
}

function NumbersSample() {
  const [value, setValue] = useState(42180);
  return (
    <Section title="Animated number">
      <div className="flex items-center gap-4">
        <AnimatedNumber
          value={value}
          className="text-3xl font-semibold tracking-tight"
        />
        <Button
          variant="outline"
          onClick={() => {
            setValue((current) => current + 5234);
          }}
        >
          Add usage
        </Button>
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

function TabsSample() {
  return (
    <Section title="Tabs">
      <Tabs defaultValue="summary">
        <TabsList>
          <TabsTrigger value="summary">Summary</TabsTrigger>
          <TabsTrigger value="sessions">Sessions</TabsTrigger>
          <TabsTrigger value="history">History</TabsTrigger>
        </TabsList>
        <TabsContent value="summary" className="text-sm text-muted-foreground">
          Summary panel
        </TabsContent>
        <TabsContent value="sessions" className="text-sm text-muted-foreground">
          Sessions panel
        </TabsContent>
        <TabsContent value="history" className="text-sm text-muted-foreground">
          History panel
        </TabsContent>
      </Tabs>
    </Section>
  );
}

function SwitchSample() {
  return (
    <Section title="Switch">
      <div className="flex items-center gap-3">
        <Switch defaultChecked aria-label="On example" />
        <Switch aria-label="Off example" />
        <Switch disabled aria-label="Disabled example" />
      </div>
    </Section>
  );
}

function CardSample() {
  return (
    <Section title="Cards">
      <div className="grid gap-4 sm:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Summary</CardTitle>
            <CardDescription>Generic card surface</CardDescription>
          </CardHeader>
          <CardContent className="text-sm text-muted-foreground">
            Card content uses the card surface token.
          </CardContent>
        </Card>
        <CompactCard className="p-5">
          <p className="text-sm text-muted-foreground">Today token usage</p>
          <p className="mt-2 text-3xl font-semibold tracking-tight">42,180</p>
          <p className="mt-1 text-xs text-muted-foreground">tokens today</p>
        </CompactCard>
      </div>
    </Section>
  );
}
