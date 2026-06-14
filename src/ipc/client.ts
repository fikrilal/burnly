import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { z } from "zod";

import { BurnlyClientError } from "./errors";
import {
  COMMAND_NAMES,
  CONTRACT_VERSION,
  type CommandInvoker,
  type CommandName,
  type CommandRequests,
  type ContractProbeResponse,
  type FieldError,
  type IpcError,
  type IpcResponse,
  type ResponseMeta,
} from "./generated/contracts";

const responseMetaSchema: z.ZodType<ResponseMeta> = z.object({
  contractVersion: z.literal(CONTRACT_VERSION),
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

export interface CommandResult<TData> {
  data: TData;
  meta: ResponseMeta;
}

export const commandInvoker: CommandInvoker = async (command, request) => {
  return tauriInvoke(command, request);
};

export async function invokeCommand<TCommand extends CommandName>(
  command: TCommand,
  request: CommandRequests[TCommand],
  invoker: CommandInvoker = commandInvoker,
): Promise<CommandResult<ContractProbeResponse>> {
  try {
    const response = await invoker(command, request);
    return unwrapResponse(response);
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
  response: IpcResponse<ContractProbeResponse>,
): IpcResponse<ContractProbeResponse> {
  const meta = responseMetaSchema.parse(response.meta);

  if (!response.ok) {
    const error = toIpcError(ipcErrorSchema.parse(response.error));
    return {
      ok: false,
      error,
      meta,
    };
  }

  return {
    ok: true,
    data: contractProbeDataSchema.parse(response.data),
    meta,
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
