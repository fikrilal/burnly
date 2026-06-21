import { useState, type ReactNode } from "react";
import {
  AlertCircle,
  CheckCircle,
  Pencil,
  Plus,
  Save,
  Trash2,
} from "lucide-react";
import * as Dialog from "@radix-ui/react-dialog";

import { BurnlyClientError } from "../../ipc/errors";
import type {
  BudgetDefinition,
  BudgetLimit,
  BudgetResponse,
  BudgetScope,
} from "../../ipc/generated/contracts";
import { formatCurrency, formatNumber } from "../../lib/format";
import {
  useBudgets,
  useCreateBudget,
  useDeleteBudget,
  useDisableBudget,
  useEnableBudget,
  useUpdateBudget,
} from "./use-budgets";

type BudgetFormMode =
  | { kind: "create" }
  | { kind: "edit"; budget: BudgetResponse };

interface BudgetDraft {
  name: string;
  metric: "tokens" | "cost";
  tokenLimit: string;
  costAmount: string;
  costCurrency: string;
  period: BudgetDefinition["period"];
  scopeKind: BudgetScope["kind"];
  sourceId: string;
  enabled: boolean;
  thresholds: string[];
}

interface DraftError {
  field: string;
  message: string;
}

export function BudgetsView() {
  const budgetsQuery = useBudgets();
  const createMutation = useCreateBudget();
  const updateMutation = useUpdateBudget();
  const enableMutation = useEnableBudget();
  const disableMutation = useDisableBudget();
  const deleteMutation = useDeleteBudget();
  const [formMode, setFormMode] = useState<BudgetFormMode>({ kind: "create" });

  if (budgetsQuery.isPending) {
    return <BudgetStatus title="Loading budgets" />;
  }

  if (budgetsQuery.isError) {
    return (
      <BudgetStatus
        title="Budgets unavailable"
        detail={errorMessage(budgetsQuery.error)}
        action={
          <button
            type="button"
            className={secondaryButtonClass}
            onClick={() => {
              void budgetsQuery.refetch();
            }}
          >
            Retry
          </button>
        }
      />
    );
  }

  const mutationError =
    createMutation.error ??
    updateMutation.error ??
    enableMutation.error ??
    disableMutation.error ??
    deleteMutation.error;

  const saved = createMutation.isSuccess || updateMutation.isSuccess;

  return (
    <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_24rem]">
      <section className="space-y-4">
        <BudgetHeader
          count={budgetsQuery.data.items.length}
          onCreate={() => {
            clearMutations();
            setFormMode({ kind: "create" });
          }}
        />
        <FormStatus
          saved={saved}
          error={mutationError}
          onReload={() => {
            clearMutations();
            void budgetsQuery.refetch();
          }}
        />
        <BudgetList
          budgets={budgetsQuery.data.items}
          busy={
            enableMutation.isPending ||
            disableMutation.isPending ||
            deleteMutation.isPending
          }
          selectedBudgetId={
            formMode.kind === "edit" ? formMode.budget.id : undefined
          }
          onEdit={(budget) => {
            clearMutations();
            setFormMode({ kind: "edit", budget });
          }}
          onEnable={(budget) => {
            clearMutations();
            enableMutation.mutate({
              budgetId: budget.id,
              expectedRevision: budget.revision,
            });
          }}
          onDisable={(budget) => {
            clearMutations();
            disableMutation.mutate({
              budgetId: budget.id,
              expectedRevision: budget.revision,
            });
          }}
          onDelete={(budget) => {
            clearMutations();
            deleteMutation.mutate(
              { budgetId: budget.id, expectedRevision: budget.revision },
              {
                onSuccess: () => {
                  if (
                    formMode.kind === "edit" &&
                    formMode.budget.id === budget.id
                  ) {
                    setFormMode({ kind: "create" });
                  }
                },
              },
            );
          }}
        />
      </section>
      <BudgetForm
        key={formKey(formMode)}
        mode={formMode}
        saving={createMutation.isPending || updateMutation.isPending}
        onSubmit={(request) => {
          clearMutations();
          if (formMode.kind === "create") {
            createMutation.mutate(
              { budget: request },
              {
                onSuccess: (budget) => {
                  setFormMode({ kind: "edit", budget });
                },
              },
            );
          } else {
            updateMutation.mutate(
              {
                budgetId: formMode.budget.id,
                expectedRevision: formMode.budget.revision,
                budget: request,
              },
              {
                onSuccess: (budget) => {
                  setFormMode({ kind: "edit", budget });
                },
              },
            );
          }
        }}
      />
    </div>
  );

  function clearMutations() {
    createMutation.reset();
    updateMutation.reset();
    enableMutation.reset();
    disableMutation.reset();
    deleteMutation.reset();
  }
}

function BudgetHeader({
  count,
  onCreate,
}: {
  count: number;
  onCreate: () => void;
}) {
  return (
    <div className="flex flex-col justify-between gap-4 sm:flex-row sm:items-end">
      <div>
        <h2 className="text-2xl font-semibold tracking-tight text-white">
          Budgets
        </h2>
        <p className="mt-1 text-sm text-zinc-400">
          {count === 0
            ? "Create token or cost guardrails for local usage."
            : `${count} configured budget${count === 1 ? "" : "s"}.`}
        </p>
      </div>
      <button type="button" className={primaryButtonClass} onClick={onCreate}>
        <Plus className="h-4 w-4" aria-hidden />
        New budget
      </button>
    </div>
  );
}

function BudgetList({
  budgets,
  busy,
  selectedBudgetId,
  onEdit,
  onEnable,
  onDisable,
  onDelete,
}: {
  budgets: BudgetResponse[];
  busy: boolean;
  selectedBudgetId: string | undefined;
  onEdit: (budget: BudgetResponse) => void;
  onEnable: (budget: BudgetResponse) => void;
  onDisable: (budget: BudgetResponse) => void;
  onDelete: (budget: BudgetResponse) => void;
}) {
  if (budgets.length === 0) {
    return (
      <div className="rounded-lg border border-zinc-800 bg-zinc-900/50 p-8 text-center">
        <p className="font-medium text-zinc-200">No budgets yet</p>
        <p className="mt-2 text-sm text-zinc-500">
          Use the form to create a daily, weekly, or monthly budget.
        </p>
      </div>
    );
  }

  return (
    <div className="grid gap-3">
      {budgets.map((budget) => (
        <article
          key={budget.id}
          className={`rounded-lg border bg-zinc-900/60 p-4 ${
            budget.id === selectedBudgetId
              ? "border-cyan-700"
              : "border-zinc-800"
          }`}
        >
          <div className="flex flex-col justify-between gap-3 sm:flex-row">
            <div>
              <div className="flex flex-wrap items-center gap-2">
                <h3 className="font-medium text-white">{budget.name}</h3>
                <StatusPill enabled={budget.enabled} />
              </div>
              <p className="mt-2 text-sm text-zinc-400">
                {limitLabel(budget.limit)} · {budget.period} ·{" "}
                {scopeLabel(budget.scope)}
              </p>
              <p className="mt-1 text-xs text-zinc-500">
                Revision {budget.revision} · thresholds{" "}
                {thresholdLabel(budget.thresholds)}
              </p>
            </div>
            <div className="flex flex-wrap items-start gap-2">
              <button
                type="button"
                className={secondaryButtonClass}
                onClick={() => {
                  onEdit(budget);
                }}
              >
                <Pencil className="h-4 w-4" aria-hidden />
                Edit
              </button>
              <button
                type="button"
                disabled={busy}
                className={secondaryButtonClass}
                onClick={() => {
                  if (budget.enabled) {
                    onDisable(budget);
                  } else {
                    onEnable(budget);
                  }
                }}
              >
                {budget.enabled ? "Disable" : "Enable"}
              </button>
              <DeleteBudgetButton
                budgetName={budget.name}
                disabled={busy}
                onConfirm={() => {
                  onDelete(budget);
                }}
              />
            </div>
          </div>
        </article>
      ))}
    </div>
  );
}

function BudgetForm({
  mode,
  saving,
  onSubmit,
}: {
  mode: BudgetFormMode;
  saving: boolean;
  onSubmit: (budget: BudgetDefinition) => void;
}) {
  const [draft, setDraft] = useState(() => draftFromMode(mode));
  const [errors, setErrors] = useState<DraftError[]>([]);
  const title = mode.kind === "create" ? "Create budget" : "Edit budget";

  return (
    <aside className="rounded-lg border border-zinc-800 bg-zinc-900/50 p-5">
      <h3 className="text-lg font-semibold text-white">{title}</h3>
      <form
        className="mt-5 space-y-5"
        onSubmit={(event) => {
          event.preventDefault();
          const result = buildBudgetDefinition(draft);
          if (result.kind === "invalid") {
            setErrors(result.errors);
            return;
          }
          setErrors([]);
          onSubmit(result.budget);
        }}
      >
        <ValidationSummary errors={errors} />
        <Field label="Name" htmlFor="budget-name">
          <input
            id="budget-name"
            value={draft.name}
            onChange={(event) => {
              setDraft({ ...draft, name: event.target.value });
            }}
            className={inputClass}
          />
        </Field>
        <Field label="Metric" htmlFor="budget-metric">
          <select
            id="budget-metric"
            value={draft.metric}
            onChange={(event) => {
              const metric = event.target.value === "cost" ? "cost" : "tokens";
              setDraft({ ...draft, metric });
            }}
            className={inputClass}
          >
            <option value="tokens">Tokens</option>
            <option value="cost">Cost</option>
          </select>
        </Field>
        {draft.metric === "tokens" ? (
          <Field label="Token limit" htmlFor="budget-token-limit">
            <input
              id="budget-token-limit"
              inputMode="numeric"
              value={draft.tokenLimit}
              onChange={(event) => {
                setDraft({ ...draft, tokenLimit: event.target.value });
              }}
              className={inputClass}
            />
          </Field>
        ) : (
          <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_6rem]">
            <Field label="Cost limit" htmlFor="budget-cost-limit">
              <input
                id="budget-cost-limit"
                inputMode="decimal"
                value={draft.costAmount}
                onChange={(event) => {
                  setDraft({ ...draft, costAmount: event.target.value });
                }}
                className={inputClass}
              />
            </Field>
            <Field label="Currency" htmlFor="budget-currency">
              <input
                id="budget-currency"
                value={draft.costCurrency}
                maxLength={3}
                onChange={(event) => {
                  setDraft({
                    ...draft,
                    costCurrency: event.target.value.toUpperCase(),
                  });
                }}
                className={inputClass}
              />
            </Field>
          </div>
        )}
        <Field label="Period" htmlFor="budget-period">
          <select
            id="budget-period"
            value={draft.period}
            onChange={(event) => {
              const period = parsePeriod(event.target.value);
              setDraft({ ...draft, period });
            }}
            className={inputClass}
          >
            <option value="daily">Daily</option>
            <option value="weekly">Weekly</option>
            <option value="monthly">Monthly</option>
          </select>
        </Field>
        <Field label="Scope" htmlFor="budget-scope">
          <select
            id="budget-scope"
            value={draft.scopeKind}
            onChange={(event) => {
              const scopeKind =
                event.target.value === "source" ? "source" : "global";
              setDraft({ ...draft, scopeKind });
            }}
            className={inputClass}
          >
            <option value="global">Global</option>
            <option value="source">Source</option>
          </select>
        </Field>
        {draft.scopeKind === "source" ? (
          <Field label="Source ID" htmlFor="budget-source-id">
            <input
              id="budget-source-id"
              inputMode="numeric"
              value={draft.sourceId}
              onChange={(event) => {
                setDraft({ ...draft, sourceId: event.target.value });
              }}
              className={inputClass}
            />
          </Field>
        ) : null}
        <ThresholdFields draft={draft} onChange={setDraft} />
        <label className="flex items-center justify-between gap-4 border-t border-zinc-800 pt-5">
          <span className="text-sm font-medium text-zinc-300">Enabled</span>
          <input
            type="checkbox"
            checked={draft.enabled}
            onChange={(event) => {
              setDraft({ ...draft, enabled: event.target.checked });
            }}
            className="h-4 w-4"
          />
        </label>
        <button type="submit" disabled={saving} className={primaryButtonClass}>
          <Save className="h-4 w-4" aria-hidden />
          {saving
            ? "Saving"
            : mode.kind === "create"
              ? "Create budget"
              : "Save budget"}
        </button>
      </form>
    </aside>
  );
}

function ThresholdFields({
  draft,
  onChange,
}: {
  draft: BudgetDraft;
  onChange: (draft: BudgetDraft) => void;
}) {
  return (
    <fieldset className="space-y-3 border-t border-zinc-800 pt-5">
      <legend className="text-sm font-medium text-zinc-300">
        Warning thresholds
      </legend>
      {draft.thresholds.map((threshold, index) => (
        <div key={index} className="flex items-center gap-2">
          <input
            aria-label={`Threshold ${index + 1}`}
            inputMode="decimal"
            value={threshold}
            onChange={(event) => {
              const thresholds = draft.thresholds.map((value, itemIndex) =>
                itemIndex === index ? event.target.value : value,
              );
              onChange({ ...draft, thresholds });
            }}
            className={inputClass}
          />
          <span className="text-sm text-zinc-500">%</span>
          <button
            type="button"
            className={secondaryButtonClass}
            onClick={() => {
              onChange({
                ...draft,
                thresholds: draft.thresholds.filter(
                  (_, itemIndex) => itemIndex !== index,
                ),
              });
            }}
          >
            Remove
          </button>
        </div>
      ))}
      <button
        type="button"
        className={secondaryButtonClass}
        onClick={() => {
          onChange({ ...draft, thresholds: [...draft.thresholds, "90"] });
        }}
      >
        Add threshold
      </button>
    </fieldset>
  );
}

function DeleteBudgetButton({
  budgetName,
  disabled,
  onConfirm,
}: {
  budgetName: string;
  disabled: boolean;
  onConfirm: () => void;
}) {
  return (
    <Dialog.Root>
      <Dialog.Trigger asChild>
        <button type="button" disabled={disabled} className={dangerButtonClass}>
          <Trash2 className="h-4 w-4" aria-hidden />
          Delete
        </button>
      </Dialog.Trigger>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/70" />
        <Dialog.Content className="fixed left-1/2 top-1/2 w-[min(28rem,calc(100vw-2rem))] -translate-x-1/2 -translate-y-1/2 border border-zinc-700 bg-zinc-950 p-6 text-zinc-100">
          <Dialog.Title className="text-lg font-semibold">
            Delete budget?
          </Dialog.Title>
          <Dialog.Description className="mt-2 text-sm text-zinc-400">
            This permanently deletes “{budgetName}”. Budget history is not
            recalculated by this action.
          </Dialog.Description>
          <div className="mt-6 flex justify-end gap-3">
            <Dialog.Close asChild>
              <button type="button" className={secondaryButtonClass}>
                Cancel
              </button>
            </Dialog.Close>
            <Dialog.Close asChild>
              <button
                type="button"
                className={dangerButtonClass}
                onClick={onConfirm}
              >
                Delete budget
              </button>
            </Dialog.Close>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function ValidationSummary({ errors }: { errors: DraftError[] }) {
  if (errors.length === 0) return null;

  return (
    <div className="border border-red-900 bg-red-950/20 p-3 text-sm text-red-300">
      <p className="font-medium">Fix the budget before saving.</p>
      <ul className="mt-2 list-disc space-y-1 pl-5">
        {errors.map((error) => (
          <li key={`${error.field}:${error.message}`}>{error.message}</li>
        ))}
      </ul>
    </div>
  );
}

function FormStatus({
  saved,
  error,
  onReload,
}: {
  saved: boolean;
  error: Error | null;
  onReload: () => void;
}) {
  if (error) {
    const isConflict =
      error instanceof BurnlyClientError && error.category === "conflict";
    return (
      <StatusMessage icon={AlertCircle} tone="error">
        {errorMessage(error)}
        {isConflict ? (
          <button type="button" className="ml-2 underline" onClick={onReload}>
            Reload budgets
          </button>
        ) : null}
      </StatusMessage>
    );
  }

  return saved ? (
    <StatusMessage icon={CheckCircle} tone="success">
      Budget saved.
    </StatusMessage>
  ) : null;
}

function Field({
  label,
  htmlFor,
  children,
}: {
  label: string;
  htmlFor: string;
  children: ReactNode;
}) {
  return (
    <div className="space-y-2">
      <label
        htmlFor={htmlFor}
        className="block text-sm font-medium text-zinc-300"
      >
        {label}
      </label>
      {children}
    </div>
  );
}

function StatusPill({ enabled }: { enabled: boolean }) {
  return (
    <span
      className={`rounded px-2 py-0.5 text-xs ${
        enabled
          ? "bg-emerald-400/10 text-emerald-300"
          : "bg-zinc-800 text-zinc-400"
      }`}
    >
      {enabled ? "Enabled" : "Disabled"}
    </span>
  );
}

function StatusMessage({
  icon: Icon,
  tone,
  children,
}: {
  icon: typeof AlertCircle;
  tone: "success" | "error";
  children: ReactNode;
}) {
  return (
    <div
      className={`flex items-center gap-2 border p-3 text-sm ${
        tone === "success"
          ? "border-emerald-900 text-emerald-400"
          : "border-red-900 text-red-400"
      }`}
    >
      <Icon className="h-4 w-4 shrink-0" aria-hidden />
      <span>{children}</span>
    </div>
  );
}

function BudgetStatus({
  title,
  detail,
  action,
}: {
  title: string;
  detail?: string;
  action?: ReactNode;
}) {
  return (
    <div className="mx-auto max-w-2xl border border-zinc-800 p-6">
      <h2 className="text-lg font-semibold">{title}</h2>
      {detail ? <p className="mt-2 text-sm text-zinc-400">{detail}</p> : null}
      {action ? <div className="mt-4">{action}</div> : null}
    </div>
  );
}

function draftFromMode(mode: BudgetFormMode): BudgetDraft {
  if (mode.kind === "create") {
    return {
      name: "",
      metric: "tokens",
      tokenLimit: "1000000",
      costAmount: "50",
      costCurrency: "USD",
      period: "monthly",
      scopeKind: "global",
      sourceId: "",
      enabled: true,
      thresholds: ["80", "100"],
    };
  }

  const { budget } = mode;
  return {
    name: budget.name,
    metric: budget.limit.kind,
    tokenLimit: budget.limit.kind === "tokens" ? budget.limit.value : "1000000",
    costAmount:
      budget.limit.kind === "cost"
        ? amountMicrosToDecimal(budget.limit.amountMicros)
        : "50",
    costCurrency: budget.limit.kind === "cost" ? budget.limit.currency : "USD",
    period: budget.period,
    scopeKind: budget.scope.kind,
    sourceId: budget.scope.kind === "source" ? budget.scope.sourceId : "",
    enabled: budget.enabled,
    thresholds: budget.thresholds.map((threshold) =>
      basisPointsToPercent(threshold.basisPoints),
    ),
  };
}

function buildBudgetDefinition(
  draft: BudgetDraft,
):
  | { kind: "valid"; budget: BudgetDefinition }
  | { kind: "invalid"; errors: DraftError[] } {
  const errors: DraftError[] = [];
  const name = draft.name.trim();

  if (!name) {
    errors.push({ field: "name", message: "Name is required." });
  }

  const limit = parseLimit(draft, errors);
  const scope = parseScope(draft, errors);
  const thresholds = parseThresholds(draft.thresholds, errors);

  if (!limit || !scope || !thresholds || errors.length > 0) {
    return { kind: "invalid", errors };
  }

  return {
    kind: "valid",
    budget: {
      name,
      limit,
      period: draft.period,
      scope,
      enabled: draft.enabled,
      thresholds,
    },
  };
}

function parseLimit(
  draft: BudgetDraft,
  errors: DraftError[],
): BudgetLimit | null {
  if (draft.metric === "tokens") {
    if (!isPositiveIntegerString(draft.tokenLimit)) {
      errors.push({
        field: "tokenLimit",
        message: "Token limit must be a positive whole number.",
      });
      return null;
    }
    return { kind: "tokens", value: draft.tokenLimit };
  }

  const amountMicros = parseDecimalMicros(draft.costAmount);
  if (!amountMicros) {
    errors.push({
      field: "costAmount",
      message: "Cost limit must be a positive decimal amount.",
    });
  }
  if (!/^[A-Z]{3}$/.test(draft.costCurrency)) {
    errors.push({
      field: "costCurrency",
      message: "Currency must be a 3-letter uppercase ISO code.",
    });
  }
  return amountMicros && /^[A-Z]{3}$/.test(draft.costCurrency)
    ? { kind: "cost", amountMicros, currency: draft.costCurrency }
    : null;
}

function parseScope(
  draft: BudgetDraft,
  errors: DraftError[],
): BudgetScope | null {
  if (draft.scopeKind === "global") return { kind: "global" };

  if (!isPositiveIntegerString(draft.sourceId)) {
    errors.push({
      field: "sourceId",
      message: "Source ID must be a positive whole number.",
    });
    return null;
  }

  return { kind: "source", sourceId: draft.sourceId };
}

function parseThresholds(
  thresholds: string[],
  errors: DraftError[],
): BudgetDefinition["thresholds"] | null {
  const parsed = thresholds.map((threshold) =>
    parsePercentBasisPoints(threshold),
  );
  if (parsed.some((threshold) => threshold === null)) {
    errors.push({
      field: "thresholds",
      message: "Thresholds must be positive percentages up to 100.",
    });
    return null;
  }

  const basisPoints = parsed.filter(
    (threshold): threshold is number => threshold !== null,
  );
  const unique = new Set(basisPoints);
  if (unique.size !== basisPoints.length) {
    errors.push({
      field: "thresholds",
      message: "Threshold percentages must be unique.",
    });
    return null;
  }

  return basisPoints
    .sort((left, right) => left - right)
    .map((basisPoint) => ({ basisPoints: basisPoint, enabled: true }));
}

function parsePeriod(value: string): BudgetDefinition["period"] {
  switch (value) {
    case "daily":
    case "weekly":
    case "monthly":
      return value;
    default:
      return "monthly";
  }
}

function parseDecimalMicros(value: string): string | null {
  const trimmed = value.trim();
  const match = /^([0-9]+)(?:\.([0-9]{1,6}))?$/.exec(trimmed);
  if (!match) return null;

  const whole = match[1];
  const fraction = match[2] ?? "";
  if (!whole) return null;

  const amount = BigInt(whole) * 1_000_000n + BigInt(fraction.padEnd(6, "0"));
  if (amount <= 0n || amount > 9_223_372_036_854_775_807n) return null;
  return amount.toString();
}

function parsePercentBasisPoints(value: string): number | null {
  const trimmed = value.trim();
  if (!/^(?:100(?:\.0{1,2})?|[1-9]?[0-9](?:\.[0-9]{1,2})?)$/.test(trimmed)) {
    return null;
  }

  const parsed = Number(trimmed);
  const basisPoints = Math.round(parsed * 100);
  return basisPoints > 0 && basisPoints <= 10000 ? basisPoints : null;
}

function isPositiveIntegerString(value: string): boolean {
  return /^[1-9][0-9]*$/.test(value.trim());
}

function formKey(mode: BudgetFormMode): string {
  return mode.kind === "create"
    ? "create"
    : `${mode.budget.id}:${mode.budget.revision}`;
}

function limitLabel(limit: BudgetLimit): string {
  return limit.kind === "tokens"
    ? `${formatNumber(limit.value)} tokens`
    : formatCurrency(limit.amountMicros, limit.currency);
}

function scopeLabel(scope: BudgetScope): string {
  return scope.kind === "global" ? "global" : `source ${scope.sourceId}`;
}

function thresholdLabel(thresholds: BudgetDefinition["thresholds"]): string {
  if (thresholds.length === 0) return "none";
  return thresholds
    .map((threshold) => basisPointsToPercent(threshold.basisPoints))
    .join("%, ")
    .concat("%");
}

function basisPointsToPercent(basisPoints: number): string {
  const whole = Math.floor(basisPoints / 100);
  const remainder = basisPoints % 100;
  if (remainder === 0) return String(whole);
  return `${whole}.${String(remainder).padStart(2, "0").replace(/0$/, "")}`;
}

function amountMicrosToDecimal(amountMicros: string): string {
  const amount = BigInt(amountMicros);
  const whole = amount / 1_000_000n;
  const fraction = amount % 1_000_000n;
  const fractionText = fraction.toString().padStart(6, "0").replace(/0+$/, "");
  return fractionText ? `${whole}.${fractionText}` : whole.toString();
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : "Budget action failed.";
}

const inputClass =
  "w-full border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-white focus:outline-none focus:ring-1 focus:ring-cyan-500";
const primaryButtonClass =
  "inline-flex items-center justify-center gap-2 bg-cyan-600 px-4 py-2 text-sm font-medium text-white hover:bg-cyan-500 disabled:opacity-50";
const secondaryButtonClass =
  "inline-flex items-center justify-center gap-2 border border-zinc-700 px-3 py-2 text-sm text-zinc-200 hover:border-zinc-500 disabled:opacity-50";
const dangerButtonClass =
  "inline-flex items-center justify-center gap-2 border border-red-900 px-3 py-2 text-sm text-red-300 hover:border-red-700 disabled:opacity-50";
