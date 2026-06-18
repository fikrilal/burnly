import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { z } from "zod";

import { BurnlyClientError } from "./errors";
import {
  COMMAND_NAMES,
  CONTRACT_VERSION,
  invokeAppGetBootstrap,
  invokeAppGetCapabilities,
  invokeSettingsGet,
  invokeSettingsUpdate,
  invokeSettingsUpdateProjectPathRetention,
  invokeRefreshCancel,
  invokeRefreshGetState,
  invokeRefreshRequest,
  invokeUsageGetOverview,
  invokeUsageGetCalendar,
  invokeUsageGetDayDetail,
  invokeUsageGetSessions,
  invokeUsageGetSessionDetail,
  type AppBootstrapResponse,
  type AppCapabilitiesResponse,
  type CommandInvoker,
  type CommandName,
  type CommandRequests,
  type ContractProbeResponse,
  type FieldError,
  type IpcError,
  type IpcResponse,
  type RefreshStatusResponse,
  type ResponseMeta,
  type UsageOverviewCostResponse,
  type UsageOverviewRequest,
  type UsageOverviewResponse,
  type ActivityCalendarRequest,
  type ActivityCalendarResponse,
  type DayDetailRequest,
  type DayDetailResponse,
  type SessionListRequest,
  type SessionListResponse,
  type SessionDetailRequest,
  type SessionDetailResponse,
  type SettingsResponse,
  type ProjectPathRetentionResponse,
  type UpdateProjectPathRetentionRequest,
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
    reportingTimezone: z.string().min(1),
    backgroundRefreshEnabled: z.boolean(),
    refreshIntervalMinutes: z.number().int().positive(),
    launchAtLogin: z.boolean(),
    closeBehavior: z.enum(["hide", "quit"]),
    notificationsEnabled: z.boolean(),
    storeProjectPaths: z.boolean(),
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
  nativeNotifications: capabilitySchema,
  updates: capabilitySchema,
  exportFormats: z.array(z.string().min(1)),
  diagnostics: z.object({
    desktopEvidence: z.boolean(),
  }),
});

const settingsDataSchema: z.ZodType<SettingsResponse> = z.object({
  reportingTimezone: z.string().min(1),
  backgroundRefreshEnabled: z.boolean(),
  refreshIntervalMinutes: z.number().int().min(5).max(1440),
  launchAtLogin: z.boolean(),
  closeBehavior: z.enum(["hide", "quit"]),
  notificationsEnabled: z.boolean(),
  storeProjectPaths: z.boolean(),
  revision: z.number().int().positive(),
});

const projectPathRetentionDataSchema: z.ZodType<ProjectPathRetentionResponse> =
  z.object({
    settings: settingsDataSchema,
    clearedPaths: z.number().int().nonnegative(),
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

const uint64StringSchema = z
  .string()
  .regex(/^(0|[1-9][0-9]*)$/, "Expected a canonical unsigned integer string.");

const usageOverviewCostSchema: z.ZodType<UsageOverviewCostResponse> =
  z.discriminatedUnion("valuation", [
    z.object({
      amountMicros: uint64StringSchema,
      currency: z.string().regex(/^[A-Z]{3}$/),
      valuation: z.enum(["available", "estimated"]),
      completeness: z.enum(["complete", "partial"]),
      unavailableDays: z.number().int().nonnegative(),
    }),
    z.object({
      amountMicros: uint64StringSchema.nullable(),
      currency: z
        .string()
        .regex(/^[A-Z]{3}$/)
        .nullable(),
      valuation: z.literal("unavailable"),
      completeness: z.literal("unavailable"),
      unavailableDays: z.number().int().nonnegative(),
    }),
  ]);

const usageOverviewRequestSchema: z.ZodType<UsageOverviewRequest> = z.object({
  startDate: z.iso.date(),
  endDate: z.iso.date(),
  reportingTimezone: z.string().trim().min(1),
});

const usageOverviewDataSchema: z.ZodType<UsageOverviewResponse> = z.object({
  period: usageOverviewRequestSchema,
  totalTokens: uint64StringSchema,
  activeDays: z.number().int().nonnegative(),
  cost: usageOverviewCostSchema,
  sources: z.array(
    z.object({
      source: z.string().min(1),
      totalTokens: uint64StringSchema,
      activeDays: z.number().int().nonnegative(),
      cost: usageOverviewCostSchema,
      hasPartialData: z.boolean(),
    }),
  ),
  models: z.array(
    z.object({
      name: z.string().min(1),
      totalTokens: uint64StringSchema,
      cost: usageOverviewCostSchema,
    }),
  ),
  asOf: z.iso.datetime({ offset: true }),
  lastSuccessfulRefreshAt: z.iso.datetime({ offset: true }).nullable(),
  dataStatus: z.enum(["current", "stale", "partial", "empty"]),
});

const activityCalendarRequestSchema: z.ZodType<ActivityCalendarRequest> =
  z.object({
    startDate: z.iso.date(),
    endDate: z.iso.date(),
    reportingTimezone: z.string().trim().min(1),
  });

const activityCalendarDataSchema: z.ZodType<ActivityCalendarResponse> =
  z.object({
    days: z.array(
      z.object({
        date: z.iso.date(),
        totalTokens: uint64StringSchema,
        activeSources: z.number().int().min(0),
        cost: usageOverviewCostSchema,
        hasPartialData: z.boolean(),
      }),
    ),
    dataStatus: z.enum(["current", "stale", "partial", "empty"]),
  });

const dayDetailRequestSchema: z.ZodType<DayDetailRequest> = z.object({
  date: z.iso.date(),
  reportingTimezone: z.string().trim().min(1),
});

const dayDetailDataSchema: z.ZodType<DayDetailResponse> = z.object({
  date: z.iso.date(),
  totalTokens: uint64StringSchema,
  cost: usageOverviewCostSchema,
  models: z.array(
    z.object({
      source: z.string(),
      model: z.string(),
      tokens: uint64StringSchema,
      cost: usageOverviewCostSchema,
    }),
  ),
  asOf: z.iso.datetime({ offset: true }),
});

const sessionItemResponseSchema = z.object({
  id: z.string().min(1),
  sourceId: z.string().min(1),
  label: z.string().min(1),
  projectPath: z.string().nullable(),
  firstActivityAt: z.iso.datetime({ offset: true }).nullable(),
  lastActivityAt: z.iso.datetime({ offset: true }).nullable(),
  totalTokens: uint64StringSchema,
  cost: usageOverviewCostSchema,
});

const sessionListRequestSchema: z.ZodType<SessionListRequest> = z.object({
  sourceId: z.string().min(1).nullable(),
  limit: z.number().int().positive().max(100),
  afterCursor: z.string().min(1).nullable(),
});

const sessionListDataSchema: z.ZodType<SessionListResponse> = z.object({
  items: z.array(sessionItemResponseSchema),
  nextCursor: z.string().min(1).nullable(),
});

const sessionDetailRequestSchema: z.ZodType<SessionDetailRequest> = z.object({
  sessionId: z.string().min(1),
});

const sessionDetailDataSchema: z.ZodType<SessionDetailResponse> = z.object({
  session: sessionItemResponseSchema,
  models: z.array(
    z.object({
      rawModelId: z.string().nullable(),
      totalTokens: uint64StringSchema,
      cost: usageOverviewCostSchema,
    }),
  ),
});

export interface CommandResult<TData> {
  data: TData;
  meta: ResponseMeta;
}

export const commandInvoker: CommandInvoker = async (command, request) => {
  const args = Object.entries(request).reduce<Record<string, unknown>>(
    (acc, [key, val]) => {
      acc[key] = val;
      return acc;
    },
    {},
  );
  return tauriInvoke(command, args);
};

export async function invokeCommand<TCommand extends CommandName>(
  command: TCommand,
  request: CommandRequests[TCommand],
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<unknown>> {
  try {
    const response = await invoker(command, request);
    return unwrapResponse(validateUnknownResponse(response));
  } catch (error) {
    if (error instanceof BurnlyClientError) {
      throw error;
    }

    throw transportError(error);
  }
}

export async function getContractProbe(
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<ContractProbeResponse>> {
  const response = await invoker(COMMAND_NAMES.contractProbe, {});
  const parsed = validateContractProbeResponse(response);
  return unwrapResponse(parsed);
}

export async function getAppBootstrap(
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<AppBootstrapResponse>> {
  const response = await invokeAppGetBootstrap(invoker);
  const parsed = validateBootstrapResponse(response);
  return unwrapResponse(parsed);
}

export async function getAppCapabilities(
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<AppCapabilitiesResponse>> {
  const response = await invokeAppGetCapabilities(invoker);
  const parsed = validateCapabilitiesResponse(response);
  return unwrapResponse(parsed);
}

export async function getSettings(
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<SettingsResponse>> {
  const response = await invokeSettingsGet(invoker);
  return unwrapResponse(validateResponse(response, settingsDataSchema));
}

export async function updateSettings(
  request: UpdateSettingsRequest,
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<SettingsResponse>> {
  const parsedRequest = settingsDataSchema.parse({
    ...request,
    revision: request.expectedRevision,
  });
  const response = await invokeSettingsUpdate(invoker, {
    request: {
      expectedRevision: parsedRequest.revision,
      reportingTimezone: parsedRequest.reportingTimezone,
      backgroundRefreshEnabled: parsedRequest.backgroundRefreshEnabled,
      refreshIntervalMinutes: parsedRequest.refreshIntervalMinutes,
      launchAtLogin: parsedRequest.launchAtLogin,
      closeBehavior: parsedRequest.closeBehavior,
      notificationsEnabled: parsedRequest.notificationsEnabled,
      storeProjectPaths: parsedRequest.storeProjectPaths,
    },
  });
  return unwrapResponse(validateResponse(response, settingsDataSchema));
}

export async function updateProjectPathRetention(
  request: UpdateProjectPathRetentionRequest,
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<ProjectPathRetentionResponse>> {
  const parsed = z
    .object({
      expectedRevision: z.number().int().positive(),
      retainPaths: z.boolean(),
    })
    .parse(request);
  const response = await invokeSettingsUpdateProjectPathRetention(invoker, {
    request: parsed,
  });
  return unwrapResponse(
    validateResponse(response, projectPathRetentionDataSchema),
  );
}

export async function getRefreshState(
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<RefreshStatusResponse>> {
  const response = await invokeRefreshGetState(invoker);
  return unwrapResponse(validateResponse(response, refreshStatusDataSchema));
}

export async function requestRefresh(
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<RefreshStatusResponse>> {
  const response = await invokeRefreshRequest(invoker);
  return unwrapResponse(validateResponse(response, refreshStatusDataSchema));
}

export async function cancelRefresh(
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<RefreshStatusResponse>> {
  const response = await invokeRefreshCancel(invoker);
  return unwrapResponse(validateResponse(response, refreshStatusDataSchema));
}

export async function getUsageOverview(
  request: UsageOverviewRequest,
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<UsageOverviewResponse>> {
  const parsedRequest = usageOverviewRequestSchema.parse(request);
  const response = await invokeUsageGetOverview(invoker, {
    request: parsedRequest,
  });
  return unwrapResponse(validateResponse(response, usageOverviewDataSchema));
}

export async function getActivityCalendar(
  request: ActivityCalendarRequest,
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<ActivityCalendarResponse>> {
  const parsedRequest = activityCalendarRequestSchema.parse(request);
  const response = await invokeUsageGetCalendar(invoker, {
    request: parsedRequest,
  });
  return unwrapResponse(validateResponse(response, activityCalendarDataSchema));
}

export async function getDayDetail(
  request: DayDetailRequest,
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<DayDetailResponse>> {
  const parsedRequest = dayDetailRequestSchema.parse(request);
  const response = await invokeUsageGetDayDetail(invoker, {
    request: parsedRequest,
  });
  return unwrapResponse(validateResponse(response, dayDetailDataSchema));
}

export async function getSessions(
  request: SessionListRequest,
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<SessionListResponse>> {
  const parsedRequest = sessionListRequestSchema.parse(request);
  const response = await invokeUsageGetSessions(invoker, {
    request: parsedRequest,
  });
  return unwrapResponse(validateResponse(response, sessionListDataSchema));
}

export async function getSessionDetail(
  request: SessionDetailRequest,
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<SessionDetailResponse | null>> {
  const parsedRequest = sessionDetailRequestSchema.parse(request);
  const response = await invokeUsageGetSessionDetail(invoker, {
    request: parsedRequest,
  });
  return unwrapResponse(
    validateResponse(response, sessionDetailDataSchema.nullable()),
  );
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

function validateUnknownResponse(response: unknown): IpcResponse<unknown> {
  return validateResponse(response, z.unknown());
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
