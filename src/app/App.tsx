import { Activity, Database, ShieldCheck } from "lucide-react";

const foundationItems = [
  {
    icon: ShieldCheck,
    label: "Harness",
    value: "Phase 0",
  },
  {
    icon: Database,
    label: "Storage",
    value: "SQLite next",
  },
  {
    icon: Activity,
    label: "Collector",
    value: "ccusage planned",
  },
] as const;

export function App() {
  return (
    <main className="min-h-screen bg-zinc-950 text-zinc-50">
      <section className="mx-auto flex min-h-screen w-full max-w-6xl flex-col justify-center px-6 py-10">
        <div className="max-w-3xl">
          <p className="text-sm font-medium uppercase tracking-[0.18em] text-cyan-300">
            Local AI usage tracker
          </p>
          <h1 className="mt-4 text-5xl font-semibold tracking-normal text-white">
            Burnly
          </h1>
          <p className="mt-5 max-w-2xl text-lg leading-8 text-zinc-300">
            Desktop foundation for tracking AI coding-tool token usage across
            local collectors.
          </p>
        </div>

        <div className="mt-12 grid gap-4 md:grid-cols-3">
          {foundationItems.map((item) => (
            <div
              className="rounded-lg border border-zinc-800 bg-zinc-900/70 p-5"
              key={item.label}
            >
              <item.icon className="h-5 w-5 text-cyan-300" aria-hidden />
              <p className="mt-5 text-sm text-zinc-400">{item.label}</p>
              <p className="mt-1 text-xl font-semibold text-white">
                {item.value}
              </p>
            </div>
          ))}
        </div>
      </section>
    </main>
  );
}
