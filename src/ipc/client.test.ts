import { describe, expect, it } from "vitest";
import { ZodError } from "zod";

import {
  getAppBootstrap,
  getAppCapabilities,
  getContractProbe,
  invokeCommand,
  validateInt64String,
  validateUint64String,
} from "./client";
import {
  COMMAND_NAMES,
  CONTRACT_VERSION,
  type CommandInvoker,
  type IpcResponse,
} from "./generated/contracts";

const meta = {
  contractVersion: CONTRACT_VERSION,
  requestId: "018f5f4d-7758-7bb2-9d9b-6d7f22c4a901",
  generatedAt: "2026-06-14T07:30:00.000Z",
} as const;

describe("IPC command responses", () => {
  it("unwraps successful command envelopes with metadata", async () => {
    const invoker: CommandInvoker = () => Promise.resolve(contractProbe());

    const result = await getContractProbe(invoker);

    expect(result.data.status).toBe("ok");
    expect(result.meta.requestId).toBe(meta.requestId);
  });

  it("validates bootstrap data from the desktop runtime", async () => {
    const invoker: CommandInvoker = () => Promise.resolve(bootstrap());

    const result = await getAppBootstrap(invoker);

    expect(result.data.database.status).toBe("ready");
    expect(result.data.settings.reportingTimezone).toBe("Asia/Jakarta");
    expect(result.data.sources.status).toBe("not_configured");
  });

  it("validates desktop capability data from the desktop runtime", async () => {
    const invoker: CommandInvoker = () => Promise.resolve(capabilities());

    const result = await getAppCapabilities(invoker);

    expect(result.data.tray.status).toBe("not_implemented");
    expect(result.data.exportFormats).toEqual([]);
    expect(result.data.diagnostics.desktopEvidence).toBe(true);
  });

  it("maps application error envelopes to typed client errors", async () => {
    const invoker: CommandInvoker = () => Promise.resolve(validationError());

    await expect(getContractProbe(invoker)).rejects.toMatchObject({
      kind: "application",
      code: "validation.invalid_date_range",
      requestId: meta.requestId,
      fieldErrors: [
        {
          field: "dateRange.startDate",
        },
      ],
    });
  });
});

function contractProbe(): IpcResponse<{
  status: "ok";
  contractVersion: typeof CONTRACT_VERSION;
}> {
  return {
    ok: true,
    data: {
      status: "ok",
      contractVersion: CONTRACT_VERSION,
    },
    meta,
  };
}

function bootstrap(): IpcResponse<unknown> {
  return {
    ok: true,
    data: {
      appVersion: "0.1.0",
      contractVersion: CONTRACT_VERSION,
      database: {
        status: "ready",
        schemaVersion: 1,
      },
      settings: {
        reportingTimezone: "Asia/Jakarta",
      },
      features: {
        usageOverview: false,
        collectorRefresh: false,
        budgets: false,
        settings: false,
      },
      sources: {
        status: "not_configured",
        detectedCount: 0,
        configuredCount: 0,
        enabledCount: 0,
      },
      refresh: {
        status: "idle",
        currentJobId: null,
        lastSuccessfulRefreshAt: null,
      },
      onboardingComplete: false,
    },
    meta,
  };
}

function capabilities(): IpcResponse<unknown> {
  const capability = {
    supported: false,
    status: "not_implemented",
  } as const;

  return {
    ok: true,
    data: {
      tray: capability,
      launchAtLogin: capability,
      nativeNotifications: capability,
      updates: capability,
      exportFormats: [],
      diagnostics: {
        desktopEvidence: true,
      },
    },
    meta,
  };
}

function validationError(): IpcResponse<unknown> {
  return {
    ok: false,
    error: {
      code: "validation.invalid_date_range",
      message: "The selected date range is invalid.",
      category: "validation",
      retryable: false,
      fieldErrors: [
        {
          field: "dateRange.startDate",
          code: "validation.before_end_date",
          message: "Start date must not be after end date.",
        },
      ],
      details: null,
    },
    meta,
  };
}

describe("IPC transport and validation failures", () => {
  it("maps invocation rejection to a synthetic transport error", async () => {
    const cause = new Error("command not registered");
    const invoker: CommandInvoker = () => Promise.reject(cause);

    await expect(
      invokeCommand(COMMAND_NAMES.contractProbe, {}, invoker),
    ).rejects.toMatchObject({
      kind: "transport",
      code: "transport.invoke_failed",
      category: "unavailable",
      retryable: true,
      causeValue: cause,
    });
  });

  it("rejects malformed version-sensitive payloads", async () => {
    const invoker: CommandInvoker = () =>
      Promise.resolve(
        JSON.parse(
          `{
            "ok": true,
            "data": {
              "status": "unexpected",
              "contractVersion": 1
            },
            "meta": {
              "contractVersion": 1,
              "requestId": "018f5f4d-7758-7bb2-9d9b-6d7f22c4a901",
              "generatedAt": "2026-06-14T07:30:00.000Z"
            }
          }`,
        ),
      );

    await expect(getContractProbe(invoker)).rejects.toBeInstanceOf(ZodError);
  });
});

describe("IPC integer strings", () => {
  it("parses canonical signed and unsigned integer strings exactly", () => {
    expect(validateInt64String("-9223372036854775808")).toBe(
      -9223372036854775808n,
    );
    expect(validateUint64String("18446744073709551615")).toBe(
      18446744073709551615n,
    );
  });

  it("rejects non-canonical integer strings", () => {
    expect(() => validateInt64String("01")).toThrow(TypeError);
    expect(() => validateInt64String("+1")).toThrow(TypeError);
    expect(() => validateInt64String("1.0")).toThrow(TypeError);
    expect(() => validateUint64String("-1")).toThrow(TypeError);
  });
});
