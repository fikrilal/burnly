import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import prettier from "prettier";

const root = process.cwd();
const generatedDir = path.join(root, "src", "ipc", "generated");
const generatedFile = path.join(generatedDir, "contracts.ts");
const contractSourceFile = path.join(
  root,
  "src-tauri",
  "src",
  "ipc",
  "contract.rs",
);
const ipcModuleFile = path.join(root, "src-tauri", "src", "ipc", "mod.rs");
const responseModule = path.join(
  root,
  "src-tauri",
  "src",
  "ipc",
  "response.rs",
);
const fixtureDirectory = path.join(root, "tests", "fixtures", "ipc", "v1");
const failures = [];

const shouldGenerate = process.argv.includes("--generate");

const contractSource = await readFile(contractSourceFile, "utf8");
const ipcModuleSource = await readFile(ipcModuleFile, "utf8");
const responseSource = await readFile(responseModule, "utf8");
const contract = readContractRegistry(contractSource);
const generated = await prettier.format(renderContracts(contract), {
  filepath: generatedFile,
});

if (shouldGenerate) {
  await mkdir(generatedDir, { recursive: true });
  await writeFile(generatedFile, generated);
}

const committed = await readFile(generatedFile, "utf8").catch(() => undefined);
if (committed !== generated) {
  failures.push(
    "src/ipc/generated/contracts.ts is stale. Run `pnpm contracts:generate`.",
  );
}

if (!/const CONTRACT_VERSION: u16 = 1;/.test(responseSource)) {
  failures.push("Rust IPC contract version must remain explicitly set to 1.");
}

if (!/serde\(rename_all = "camelCase"\)/.test(responseSource)) {
  failures.push("Rust IPC response DTOs must enforce camelCase wire fields.");
}

if (contract.commands.length === 0) {
  failures.push("At least one registered IPC command is required.");
}

assertUnique(
  contract.commands.map((command) => command.name),
  "command name",
);
assertUnique(
  contract.commands.map((command) => command.exportName),
  "command export",
);
assertUnique(
  contract.events.map((event) => event.name),
  "event name",
);
assertUnique(
  contract.events.map((event) => event.exportName),
  "event export",
);

for (const event of contract.events) {
  if (!event.name.startsWith("burnly://v1/")) {
    failures.push(`${event.name}: event names must be versioned under v1.`);
  }
}

for (const command of contract.commands) {
  if (!new RegExp(`\\b[a-z_]+::${command.name}\\b`).test(ipcModuleSource)) {
    failures.push(
      `${command.name}: command is missing from the Tauri invoke handler.`,
    );
  }
}

await checkResponseFixtures();

if (failures.length > 0) {
  console.error("IPC contract check failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log("IPC contract registry and generated bindings passed.");

function readContractRegistry(source) {
  return {
    commands: readSpecs(source, "CommandSpec").map((spec) => ({
      name: requireField(spec, "name", "CommandSpec"),
      exportName: requireField(spec, "export_name", "CommandSpec"),
      requestType: requireField(spec, "request_type", "CommandSpec"),
      responseType: requireField(spec, "response_type", "CommandSpec"),
    })),
    events: readSpecs(source, "EventSpec").map((spec) => ({
      name: requireField(spec, "name", "EventSpec"),
      exportName: requireField(spec, "export_name", "EventSpec"),
      payloadType: requireField(spec, "payload_type", "EventSpec"),
    })),
  };
}

function readSpecs(source, specName) {
  return [
    ...source.matchAll(new RegExp(`${specName}\\s*\\{([\\s\\S]*?)\\}`, "g")),
  ]
    .map((match) => readSpecFields(match[1] ?? ""))
    .filter((spec) => Object.keys(spec).length > 0);
}

function readSpecFields(block) {
  return Object.fromEntries(
    [...block.matchAll(/([a-z_]+):\s*"([^"]+)"/g)].map((match) => [
      match[1],
      match[2],
    ]),
  );
}

function requireField(spec, field, specName) {
  const value = spec[field];
  if (value === undefined) {
    failures.push(`${specName} is missing field ${field}.`);
    return "";
  }

  return value;
}

function renderContracts(contract) {
  const commandNameEntries = contract.commands
    .map((command) => `  ${commandKey(command.exportName)}: "${command.name}",`)
    .join("\n");
  const commandRequests = contract.commands
    .map(
      (command) =>
        `  [COMMAND_NAMES.${commandKey(command.exportName)}]: ${command.requestType};`,
    )
    .join("\n");
  const commandResponses = contract.commands
    .map(
      (command) =>
        `  [COMMAND_NAMES.${commandKey(command.exportName)}]: IpcResponse<${command.responseType}>;`,
    )
    .join("\n");
  const commandWrappers = contract.commands
    .map((command) => renderCommandWrapper(command))
    .join("\n\n");
  const eventNameEntries = contract.events
    .map((event) => `  ${event.exportName}: "${event.name}",`)
    .join("\n");
  const eventPayloads = contract.events
    .map(
      (event) => `  [EVENT_NAMES.${event.exportName}]: ${event.payloadType};`,
    )
    .join("\n");

  return `// @generated by pnpm contracts:generate
// Do not edit manually.

export const CONTRACT_VERSION = 1 as const;

export interface ResponseMeta {
  contractVersion: number;
  requestId: string;
  generatedAt: string;
}

export interface FieldError {
  field: string;
  code: string;
  message: string;
}

export type ErrorCategory =
  | "validation"
  | "conflict"
  | "not_found"
  | "collector"
  | "persistence"
  | "permission"
  | "platform"
  | "update"
  | "unavailable"
  | "internal";

export interface IpcError {
  code: string;
  message: string;
  category: ErrorCategory;
  retryable: boolean;
  fieldErrors?: FieldError[];
  details: null;
}

export type IpcResponse<TData> =
  | {
      ok: true;
      data: TData;
      meta: ResponseMeta;
    }
  | {
      ok: false;
      error: IpcError;
      meta: ResponseMeta;
    };

export interface ContractProbeResponse {
  status: "ok";
  contractVersion: number;
}

export interface AppBootstrapResponse {
  appVersion: string;
  contractVersion: number;
  database: {
    status: "ready";
    schemaVersion: number;
  };
  settings: {
    reportingTimezone: string;
    backgroundRefreshEnabled: boolean;
    refreshIntervalMinutes: number;
    launchAtLogin: boolean;
    closeBehavior: "hide" | "quit";
    notificationsEnabled: boolean;
    storeProjectPaths: boolean;
    revision: number;
  };
  features: {
    usageOverview: boolean;
    collectorRefresh: boolean;
    budgets: boolean;
    settings: boolean;
  };
  sources: {
    status: "not_configured";
    detectedCount: number;
    configuredCount: number;
    enabledCount: number;
  };
  refresh: {
    status: "idle";
    currentJobId: string | null;
    lastSuccessfulRefreshAt: string | null;
  };
  onboardingComplete: boolean;
}

export interface AppCapabilitiesResponse {
  tray: DesktopCapability;
  launchAtLogin: DesktopCapability;
  nativeNotifications: NativeNotificationCapability;
  updates: DesktopCapability;
  exportFormats: string[];
  diagnostics: {
    desktopEvidence: boolean;
  };
}

export type DiagnosticHealthStatus =
  | "healthy"
  | "degraded"
  | "unavailable"
  | "unknown";

export interface DiagnosticComponentResponse {
  component: "database" | "settings" | "sources" | "collector" | "runtime";
  status: DiagnosticHealthStatus;
  summary: string;
  details: string[];
}

export interface DiagnosticsStatusResponse {
  status: DiagnosticHealthStatus;
  contractVersion: number;
  components: DiagnosticComponentResponse[];
  logs: {
    status: "available" | "missing" | "unsupported";
    label: string;
  };
}

export interface RevealLogsResponse {
  status: "revealed" | "missing" | "unsupported";
  message: string;
}

export interface HistoryRequest {
  cursor?: string | undefined;
  limit?: number | undefined;
}

export interface HistoryCommandRequest extends Record<string, unknown> {
  request: HistoryRequest;
}

export type HistoryStatus =
  | "queued"
  | "running"
  | "stale"
  | "succeeded"
  | "partial"
  | "failed"
  | "cancelled";

export interface HistoryFailure {
  category: "collector" | "reconciliation" | "persistence" | "cancelled" | "unknown";
  retryable: boolean;
  summary: string;
}

export interface ImportHistoryItem {
  source: string;
  projection: "daily" | "session";
  scope: "full" | "incremental";
  status: HistoryStatus;
  startedAt: string;
  finishedAt: string | null;
  recordsSeen: string;
  recordsRejected: string;
  failure: HistoryFailure | null;
}

export interface RefreshHistoryItem {
  trigger: "launch" | "manual" | "scheduled" | "file_change" | "resume" | "reconcile";
  status: HistoryStatus;
  summary: string;
  startedAt: string;
  finishedAt: string | null;
  importCount: number;
  recordsSeen: string;
  recordsRejected: string;
  failure: HistoryFailure | null;
  imports: ImportHistoryItem[];
}

export interface HistoryResponse {
  items: RefreshHistoryItem[];
  nextCursor: string | null;
  limit: number;
}

export interface UpdateSettingsRequest {
  expectedRevision: number;
  reportingTimezone: string;
  backgroundRefreshEnabled: boolean;
  refreshIntervalMinutes: number;
  launchAtLogin: boolean;
  closeBehavior: "hide" | "quit";
  notificationsEnabled: boolean;
  storeProjectPaths: boolean;
}

export interface UpdateSettingsCommandRequest extends Record<string, unknown> {
  request: UpdateSettingsRequest;
}

export interface SettingsResponse {
  reportingTimezone: string;
  backgroundRefreshEnabled: boolean;
  refreshIntervalMinutes: number;
  launchAtLogin: boolean;
  closeBehavior: "hide" | "quit";
  notificationsEnabled: boolean;
  storeProjectPaths: boolean;
  revision: number;
}

export interface UpdateProjectPathRetentionRequest {
  expectedRevision: number;
  retainPaths: boolean;
}

export interface UpdateProjectPathRetentionCommandRequest
  extends Record<string, unknown> {
  request: UpdateProjectPathRetentionRequest;
}

export interface ProjectPathRetentionResponse {
  settings: SettingsResponse;
  clearedPaths: number;
}

export type BudgetLimit =
  | {
      kind: "tokens";
      value: string;
    }
  | {
      kind: "cost";
      amountMicros: string;
      currency: string;
    };

export type BudgetScope =
  | {
      kind: "global";
    }
  | {
      kind: "source";
      sourceId: string;
    };

export interface BudgetThreshold {
  basisPoints: number;
  enabled: boolean;
}

export interface BudgetDefinition {
  name: string;
  limit: BudgetLimit;
  period: "daily" | "weekly" | "monthly";
  scope: BudgetScope;
  enabled: boolean;
  thresholds: BudgetThreshold[];
}

export interface BudgetResponse extends BudgetDefinition {
  id: string;
  revision: string;
}

export interface BudgetListResponse {
  items: BudgetResponse[];
}

export interface BudgetIdRequest {
  budgetId: string;
}

export interface BudgetIdCommandRequest extends Record<string, unknown> {
  request: BudgetIdRequest;
}

export interface CreateBudgetRequest {
  budget: BudgetDefinition;
}

export interface CreateBudgetCommandRequest extends Record<string, unknown> {
  request: CreateBudgetRequest;
}

export interface UpdateBudgetRequest {
  budgetId: string;
  expectedRevision: string;
  budget: BudgetDefinition;
}

export interface UpdateBudgetCommandRequest extends Record<string, unknown> {
  request: UpdateBudgetRequest;
}

export interface MutateBudgetRequest {
  budgetId: string;
  expectedRevision: string;
}

export interface MutateBudgetCommandRequest extends Record<string, unknown> {
  request: MutateBudgetRequest;
}

export interface DeleteBudgetResponse {
  budgetId: string;
}

export interface CurrentBudgetProgressResponse {
  status: "no_budgets" | "all_disabled" | "available";
  reportingTimezone: string;
  asOf: string;
  configuredBudgetCount: number;
  enabledBudgetCount: number;
  traySummary: string | null;
  items: CurrentBudgetProgressItemResponse[];
}

export interface CurrentBudgetProgressItemResponse {
  budgetId: string;
  budgetName: string;
  period: "daily" | "weekly" | "monthly";
  periodStartDate: string;
  periodEndDate: string;
  metric: "tokens" | "cost";
  state: "available" | "cost_unavailable";
  current: string | null;
  limit: string;
  currency: string | null;
  basisPoints: string | null;
  exceeded: boolean;
  completeness: "complete" | "partial" | "unavailable";
  unavailableDays: number;
}

export interface DesktopCapability {
  supported: boolean;
  status: "available" | "not_implemented" | "unavailable";
}

export interface NativeNotificationCapability extends DesktopCapability {
  permission: "granted" | "denied" | "prompt" | "unknown";
}

export interface RefreshStatusResponse {
  status:
    | "idle"
    | "queued"
    | "running"
    | "cancelling"
    | "succeeded"
    | "partial"
    | "failed";
  jobId: string | null;
  trigger:
    | "launch"
    | "manual"
    | "scheduled"
    | "file_change"
    | "resume"
    | "reconcile"
    | null;
  lastSuccessfulRefreshAt: string | null;
}

export interface UsageOverviewRequest {
  startDate: string;
  endDate: string;
  reportingTimezone: string;
}

export interface UsageOverviewCommandRequest extends Record<string, unknown> {
  request: UsageOverviewRequest;
}

export interface UsageOverviewCostResponse {
  amountMicros: string | null;
  currency: string | null;
  valuation: "available" | "estimated" | "unavailable";
  completeness: "complete" | "partial" | "unavailable";
  unavailableDays: number;
}

export interface UsageOverviewResponse {
  period: UsageOverviewRequest;
  totalTokens: string;
  activeDays: number;
  cost: UsageOverviewCostResponse;
  sources: {
    source: string;
    totalTokens: string;
    activeDays: number;
    cost: UsageOverviewCostResponse;
    hasPartialData: boolean;
  }[];
  models: {
    name: string;
    totalTokens: string;
    cost: UsageOverviewCostResponse;
  }[];
  asOf: string;
  lastSuccessfulRefreshAt: string | null;
  dataStatus: "current" | "stale" | "partial" | "empty";
}

export interface ActivityCalendarRequest {
  startDate: string;
  endDate: string;
  reportingTimezone: string;
}

export interface ActivityCalendarCommandRequest extends Record<string, unknown> {
  request: ActivityCalendarRequest;
}

export interface ActivityCalendarDayResponse {
  date: string;
  totalTokens: string;
  activeSources: number;
  cost: UsageOverviewCostResponse;
  hasPartialData: boolean;
}

export interface ActivityCalendarResponse {
  days: ActivityCalendarDayResponse[];
  dataStatus: "current" | "stale" | "partial" | "empty";
}

export interface DayDetailRequest {
  date: string;
  reportingTimezone: string;
}

export interface DayDetailCommandRequest extends Record<string, unknown> {
  request: DayDetailRequest;
}

export interface DayDetailModelResponse {
  source: string;
  model: string;
  tokens: string;
  cost: UsageOverviewCostResponse;
}

export interface DayDetailResponse {
  date: string;
  totalTokens: string;
  cost: UsageOverviewCostResponse;
  models: DayDetailModelResponse[];
  asOf: string;
}

export interface SessionListRequest {
  sourceId: string | null;
  limit: number;
  afterCursor: string | null;
}

export interface SessionListCommandRequest extends Record<string, unknown> {
  request: SessionListRequest;
}

export interface SessionItemResponse {
  id: string;
  sourceId: string;
  label: string;
  projectPath: string | null;
  firstActivityAt: string | null;
  lastActivityAt: string | null;
  totalTokens: string;
  cost: UsageOverviewCostResponse;
}

export interface SessionListResponse {
  items: SessionItemResponse[];
  nextCursor: string | null;
}

export interface SessionDetailRequest {
  sessionId: string;
}

export interface SessionDetailCommandRequest extends Record<string, unknown> {
  request: SessionDetailRequest;
}

export interface SessionModelUsageResponse {
  rawModelId: string | null;
  totalTokens: string;
  cost: UsageOverviewCostResponse;
}

export interface SessionDetailResponse {
  session: SessionItemResponse;
  models: SessionModelUsageResponse[];
}

export type UnknownEventPayload = Record<string, unknown>;

export const COMMAND_NAMES = {
${commandNameEntries}
} as const;

export type CommandName = (typeof COMMAND_NAMES)[keyof typeof COMMAND_NAMES];

export interface CommandRequests {
${commandRequests}
}

export interface CommandResponses {
${commandResponses}
}

export type CommandInvoker = (
  command: CommandName,
  request: CommandRequests[CommandName],
) => Promise<unknown>;

${commandWrappers}

export const EVENT_NAMES = {
${eventNameEntries}
} as const;

export type EventName = (typeof EVENT_NAMES)[keyof typeof EVENT_NAMES];

export interface EventPayloads {
${eventPayloads}
}
`;
}

function commandKey(exportName) {
  return exportName
    .replace(/^invoke/, "")
    .replace(/^./, (first) => first.toLowerCase());
}

function renderCommandWrapper(command) {
  const key = commandKey(command.exportName);
  if (command.requestType === "Record<string, never>") {
    return `export function ${command.exportName}(
  invoke: CommandInvoker,
): Promise<unknown> {
  return invoke(COMMAND_NAMES.${key}, {});
}`;
  }

  return `export function ${command.exportName}(
  invoke: CommandInvoker,
  request: ${command.requestType},
): Promise<unknown> {
  return invoke(COMMAND_NAMES.${key}, request);
}`;
}

function assertUnique(values, label) {
  const seen = new Set();
  for (const value of values) {
    if (seen.has(value)) {
      failures.push(`duplicate IPC ${label}: ${value}`);
    }
    seen.add(value);
  }
}

async function checkResponseFixtures() {
  const fixtureNames = ["response-success.json", "response-error.json"];

  for (const fixtureName of fixtureNames) {
    const fixturePath = path.join(fixtureDirectory, fixtureName);
    const fixture = JSON.parse(await readFile(fixturePath, "utf8"));

    if (fixture.meta?.contractVersion !== 1) {
      failures.push(`${fixtureName}: contractVersion must be 1.`);
    }

    if (typeof fixture.meta?.requestId !== "string") {
      failures.push(`${fixtureName}: requestId must be present.`);
    }

    if (!fixture.meta?.generatedAt?.endsWith("Z")) {
      failures.push(`${fixtureName}: generatedAt must be UTC.`);
    }
  }
}
