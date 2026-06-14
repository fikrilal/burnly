import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

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
const generated = renderContracts(contract);

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
  if (!ipcModuleSource.includes(`commands::${command.name}`)) {
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
    .map(
      (command) => `export function ${command.exportName}(
  invoke: CommandInvoker,
): Promise<IpcResponse<${command.responseType}>> {
  return invoke(COMMAND_NAMES.${commandKey(command.exportName)}, {});
}`,
    )
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
  contractVersion: typeof CONTRACT_VERSION;
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
  contractVersion: typeof CONTRACT_VERSION;
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

export type CommandInvoker = <TCommand extends CommandName>(
  command: TCommand,
  request: CommandRequests[TCommand],
) => Promise<CommandResponses[TCommand]>;

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
