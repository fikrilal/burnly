import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { z } from "zod";

import { BurnlyClientError } from "./errors";
import {
  COMMAND_NAMES,
  CONTRACT_VERSION,
  invokeAppGetBootstrap,
  invokeAppGetCapabilities,
  invokeAppHideTrayPanel,
  invokeSettingsGet,
  invokeSettingsUpdate,
  invokeRefreshCancel,
  invokeRefreshGetState,
  invokeRefreshRequest,
  invokeUsageGetTraySummary,
  invokeUpdateCheck,
  invokeUpdateDownload,
  invokeUpdateGetState,
  invokeUpdateRestart,
  type AppBootstrapResponse,
  type AppCapabilitiesResponse,
  type CommandInvoker,
  type CommandName,
  type CommandRequests,
  type ContractProbeResponse,
  type HideTrayPanelResponse,
  type FieldError,
  type IpcError,
  type IpcResponse,
  type RefreshStatusResponse,
  type ResponseMeta,
  type TraySummaryRequest,
  type TraySummaryResponse,
  type SettingsResponse,
  type UpdateStatusResponse,
  type UpdateSettingsRequest,
} from "./generated/contracts";

const responseMetaSchema: z.ZodType<ResponseMeta> = z.object({
  contractVersion: z.number().int().positive(),
  requestId: z.string().min(1),
  generatedAt: z.iso.datetime({ offset: true }),
});

interface ParsedIpcError {
  code: string;
  message: string;
  category: IpcError["category"];
  retryable: boolean;
  fieldErrors?: FieldError[] | undefined;
  details: null;
}

const ipcErrorSchema: z.ZodType<ParsedIpcError> = z.object({
  code: z.string().min(1),
  message: z.string().min(1),
  category: z.enum([
    "validation",
    "conflict",
    "not_found",
    "collector",
    "persistence",
    "permission",
    "platform",
    "update",
    "unavailable",
    "internal",
  ]),
  retryable: z.boolean(),
  fieldErrors: z
    .array(
      z.object({
        field: z.string().min(1),
        code: z.string().min(1),
        message: z.string().min(1),
      }),
    )
    .optional(),
  details: z.null(),
});

const contractProbeDataSchema = z.object({
  status: z.literal("ok"),
  contractVersion: z.literal(CONTRACT_VERSION),
});

const bootstrapDataSchema: z.ZodType<AppBootstrapResponse> = z.object({
  appVersion: z.string().min(1),
  contractVersion: z.number().int().positive(),
  database: z.object({
    status: z.literal("ready"),
    schemaVersion: z.number().int().nonnegative(),
  }),
  settings: z.object({
    launchAtLogin: z.boolean(),
    closeBehavior: z.enum(["hide", "quit"]),
    revision: z.number().int().positive(),
  }),
  features: z.object({
    usageOverview: z.boolean(),
    collectorRefresh: z.boolean(),
    budgets: z.boolean(),
    settings: z.boolean(),
  }),
  sources: z.object({
    status: z.literal("not_configured"),
    detectedCount: z.number().int().nonnegative(),
    configuredCount: z.number().int().nonnegative(),
    enabledCount: z.number().int().nonnegative(),
  }),
  refresh: z.object({
    status: z.literal("idle"),
    currentJobId: z.string().min(1).nullable(),
    lastSuccessfulRefreshAt: z.iso.datetime({ offset: true }).nullable(),
  }),
  onboardingComplete: z.boolean(),
});

const capabilitySchema = z.object({
  supported: z.boolean(),
  status: z.enum(["available", "not_implemented", "unavailable"]),
});

const capabilitiesDataSchema: z.ZodType<AppCapabilitiesResponse> = z.object({
  tray: capabilitySchema,
  launchAtLogin: capabilitySchema,
  update: capabilitySchema,
  exportFormats: z.array(z.string()),
  diagnostics: z.object({
    desktopEvidence: z.boolean(),
  }),
});

const hideTrayPanelDataSchema = z.object({
  status: z.literal("hidden"),
});

const settingsDataSchema: z.ZodType<SettingsResponse> = z.object({
  launchAtLogin: z.boolean(),
  closeBehavior: z.enum(["hide", "quit"]),
  revision: z.number().int().positive(),
});

const refreshStatusDataSchema: z.ZodType<RefreshStatusResponse> = z.object({
  status: z.enum([
    "idle",
    "queued",
    "running",
    "cancelling",
    "succeeded",
    "partial",
    "failed",
  ]),
  jobId: z.string().min(1).nullable(),
  trigger: z
    .enum([
      "launch",
      "manual",
      "scheduled",
      "file_change",
      "resume",
      "reconcile",
    ])
    .nullable(),
  lastSuccessfulRefreshAt: z.iso.datetime({ offset: true }).nullable(),
});

const updateStatusDataSchema: z.ZodType<UpdateStatusResponse> = z.object({
  status: z.enum([
    "unavailable",
    "idle",
    "checking",
    "available",
    "downloading",
    "ready",
    "failed",
  ]),
  availableVersion: z.string().min(1).nullable(),
  downloadedVersion: z.string().min(1).nullable(),
  lastCheckedAt: z.iso.datetime({ offset: true }).nullable(),
  error: z
    .object({
      code: z.string().min(1),
      retryable: z.boolean(),
    })
    .nullable(),
});

const traySummaryPeriodMetricSchema = z.object({
  startDate: z.string().min(1),
  endDate: z.string().min(1),
  totalTokens: z.string().min(1),
});

const traySummaryModelSchema = z.object({
  modelName: z.string().min(1),
  agentLabel: z.string().min(1),
  totalTokens: z.string().min(1),
  trend: z
    .object({
      direction: z.enum(["increased", "decreased", "flat"]),
      basisPoints: z.number().int().nonnegative(),
    })
    .nullable(),
});

const traySummaryDataSchema: z.ZodType<TraySummaryResponse> = z.object({
  today: traySummaryPeriodMetricSchema,
  week: traySummaryPeriodMetricSchema,
  month: traySummaryPeriodMetricSchema,
  models: z.array(traySummaryModelSchema),
  asOf: z.iso.datetime({ offset: true }),
  lastSuccessfulRefreshAt: z.iso.datetime({ offset: true }).nullable(),
  dataStatus: z.enum(["current", "stale", "partial", "empty"]),
});

const updateSettingsRequestSchema = z.object({
  launchAtLogin: z.boolean(),
  closeBehavior: z.enum(["hide", "quit"]),
  expectedRevision: z.number().int().positive(),
});

const traySummaryRequestSchema = z.object({
  reportingTimezone: z.string().min(1),
});

export interface CommandResult<TData> {
  data: TData;
  meta: ResponseMeta;
}

export function commandInvoker(
  name: CommandName,
  body: CommandRequests[CommandName],
): Promise<unknown> {
  return tauriInvoke(name, body);
}

export async function probeContract(
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<ContractProbeResponse>> {
  try {
    const response = await invoker("__burnly_contract_probe", {});
    return unwrapResponse(validateContractProbeResponse(response));
  } catch (error) {
    if (error instanceof BurnlyClientError) throw error;
    throw transportError(error);
  }
}

export async function getAppBootstrap(
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<AppBootstrapResponse>> {
  try {
    const response = await invokeAppGetBootstrap(invoker);
    return unwrapResponse(validateBootstrapResponse(response));
  } catch (error) {
    if (error instanceof BurnlyClientError) throw error;
    throw transportError(error);
  }
}

export async function getAppCapabilities(
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<AppCapabilitiesResponse>> {
  try {
    const response = await invokeAppGetCapabilities(invoker);
    return unwrapResponse(validateCapabilitiesResponse(response));
  } catch (error) {
    if (error instanceof BurnlyClientError) throw error;
    throw transportError(error);
  }
}

export async function hideTrayPanel(
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<HideTrayPanelResponse>> {
  try {
    const response = await invokeAppHideTrayPanel(invoker);
    return unwrapResponse(validateResponse(response, hideTrayPanelDataSchema));
  } catch (error) {
    if (error instanceof BurnlyClientError) throw error;
    throw transportError(error);
  }
}

export async function getSettings(
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<SettingsResponse>> {
  try {
    const response = await invokeSettingsGet(invoker);
    return unwrapResponse(validateResponse(response, settingsDataSchema));
  } catch (error) {
    if (error instanceof BurnlyClientError) throw error;
    throw transportError(error);
  }
}

export async function updateSettings(
  request: UpdateSettingsRequest,
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<SettingsResponse>> {
  try {
    const parsedRequest = updateSettingsRequestSchema.parse(request);
    const response = await invokeSettingsUpdate(invoker, {
      request: parsedRequest,
    });
    return unwrapResponse(validateResponse(response, settingsDataSchema));
  } catch (error) {
    if (error instanceof BurnlyClientError) throw error;
    throw transportError(error);
  }
}

export async function getRefreshState(
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<RefreshStatusResponse>> {
  try {
    const response = await invokeRefreshGetState(invoker);
    return unwrapResponse(validateResponse(response, refreshStatusDataSchema));
  } catch (error) {
    if (error instanceof BurnlyClientError) throw error;
    throw transportError(error);
  }
}

export async function requestRefresh(
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<RefreshStatusResponse>> {
  try {
    const response = await invokeRefreshRequest(invoker);
    return unwrapResponse(validateResponse(response, refreshStatusDataSchema));
  } catch (error) {
    if (error instanceof BurnlyClientError) throw error;
    throw transportError(error);
  }
}

export async function cancelRefresh(
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<RefreshStatusResponse>> {
  try {
    const response = await invokeRefreshCancel(invoker);
    return unwrapResponse(validateResponse(response, refreshStatusDataSchema));
  } catch (error) {
    if (error instanceof BurnlyClientError) throw error;
    throw transportError(error);
  }
}

export async function getUpdateState(
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<UpdateStatusResponse>> {
  try {
    const response = await invokeUpdateGetState(invoker);
    return unwrapResponse(validateResponse(response, updateStatusDataSchema));
  } catch (error) {
    if (error instanceof BurnlyClientError) throw error;
    throw transportError(error);
  }
}

export async function checkForUpdate(
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<UpdateStatusResponse>> {
  try {
    const response = await invokeUpdateCheck(invoker);
    return unwrapResponse(validateResponse(response, updateStatusDataSchema));
  } catch (error) {
    if (error instanceof BurnlyClientError) throw error;
    throw transportError(error);
  }
}

export async function downloadUpdate(
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<UpdateStatusResponse>> {
  try {
    const response = await invokeUpdateDownload(invoker);
    return unwrapResponse(validateResponse(response, updateStatusDataSchema));
  } catch (error) {
    if (error instanceof BurnlyClientError) throw error;
    throw transportError(error);
  }
}

export async function restartForUpdate(
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<UpdateStatusResponse>> {
  try {
    const response = await invokeUpdateRestart(invoker);
    return unwrapResponse(validateResponse(response, updateStatusDataSchema));
  } catch (error) {
    if (error instanceof BurnlyClientError) throw error;
    throw transportError(error);
  }
}

export async function getTraySummary(
  request: TraySummaryRequest,
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<TraySummaryResponse>> {
  try {
    const parsedRequest = traySummaryRequestSchema.parse(request);
    const response = await invokeUsageGetTraySummary(invoker, {
      request: parsedRequest,
    });
    return unwrapResponse(validateResponse(response, traySummaryDataSchema));
  } catch (error) {
    if (error instanceof BurnlyClientError) throw error;
    throw transportError(error);
  }
}

export function validateInt64String(value: string): bigint {
  if (!/^-?(0|[1-9][0-9]*)$/.test(value)) {
    throw new TypeError("Invalid Int64String.");
  }

  return BigInt(value);
}

export function validateUint64String(value: string): bigint {
  const parsed = validateInt64String(value);
  if (parsed < 0n) {
    throw new TypeError("Invalid non-negative Int64String.");
  }

  return parsed;
}

function unwrapResponse<TData>(
  response: IpcResponse<TData>,
): CommandResult<TData> {
  const meta = responseMetaSchema.parse(response.meta);

  if (response.ok) {
    return {
      data: response.data,
      meta,
    };
  }

  const error = toIpcError(ipcErrorSchema.parse(response.error));
  throw new BurnlyClientError({
    kind: "application",
    error,
    requestId: meta.requestId,
    generatedAt: meta.generatedAt,
  });
}

function validateContractProbeResponse(
  response: unknown,
): IpcResponse<ContractProbeResponse> {
  return validateResponse(response, contractProbeDataSchema);
}

function validateBootstrapResponse(
  response: unknown,
): IpcResponse<AppBootstrapResponse> {
  return validateResponse(response, bootstrapDataSchema);
}

function validateCapabilitiesResponse(
  response: unknown,
): IpcResponse<AppCapabilitiesResponse> {
  return validateResponse(response, capabilitiesDataSchema);
}

function validateResponse<TData>(
  response: unknown,
  dataSchema: z.ZodType<TData>,
): IpcResponse<TData> {
  const envelope = z
    .object({
      ok: z.boolean(),
      meta: responseMetaSchema,
    })
    .parse(response);

  if (!envelope.ok) {
    const failure = z
      .object({
        ok: z.literal(false),
        error: ipcErrorSchema,
        meta: responseMetaSchema,
      })
      .parse(response);
    const error = toIpcError(failure.error);
    return {
      ok: false,
      error,
      meta: failure.meta,
    };
  }

  const success = z
    .object({
      ok: z.literal(true),
      data: dataSchema,
      meta: responseMetaSchema,
    })
    .parse(response);

  return {
    ok: true,
    data: success.data,
    meta: success.meta,
  };
}

function transportError(cause: unknown): BurnlyClientError {
  return new BurnlyClientError({
    kind: "transport",
    error: {
      code: "transport.invoke_failed",
      message: "Burnly could not reach the desktop runtime.",
      category: "unavailable",
      retryable: true,
      details: null,
    },
    requestId: crypto.randomUUID(),
    generatedAt: new Date().toISOString(),
    cause,
  });
}

function toIpcError(error: ParsedIpcError): IpcError {
  if (error.fieldErrors === undefined) {
    return {
      code: error.code,
      message: error.message,
      category: error.category,
      retryable: error.retryable,
      details: error.details,
    };
  }

  return {
    code: error.code,
    message: error.message,
    category: error.category,
    retryable: error.retryable,
    fieldErrors: error.fieldErrors,
    details: error.details,
  };
}

export { COMMAND_NAMES, CONTRACT_VERSION };
