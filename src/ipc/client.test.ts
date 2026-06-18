import { describe, expect, it, vi } from "vitest";
import { ZodError } from "zod";

import {
  getAppBootstrap,
  getAppCapabilities,
  createBudget,
  deleteBudget,
  disableBudget,
  enableBudget,
  getBudget,
  listBudgets,
  getContractProbe,
  getRefreshState,
  getSettings,
  getUsageOverview,
  invokeCommand,
  validateInt64String,
  validateUint64String,
  updateSettings,
  updateProjectPathRetention,
  updateBudget,
} from "./client";
import {
  COMMAND_NAMES,
  CONTRACT_VERSION,
  type BudgetDefinition,
  type CommandInvoker,
  type IpcResponse,
  type UsageOverviewResponse,
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

  it("validates refresh state from the desktop runtime", async () => {
    const invoker: CommandInvoker = () => Promise.resolve(refreshState());

    const result = await getRefreshState(invoker);

    expect(result.data.status).toBe("succeeded");
    expect(result.data.jobId).toBe("refresh-1000-0");
    expect(result.data.trigger).toBe("manual");
    expect(result.data.lastSuccessfulRefreshAt).toBe(
      "2026-06-15T00:00:00+00:00",
    );
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

describe("settings IPC", () => {
  it("gets and updates revisioned settings through dedicated commands", async () => {
    const invoker: CommandInvoker = (command, request) => {
      if (command === COMMAND_NAMES.settingsGet) {
        expect(request).toEqual({});
        return Promise.resolve(settingsResponse(1));
      }
      expect(command).toBe(COMMAND_NAMES.settingsUpdate);
      expect(request).toMatchObject({
        request: {
          expectedRevision: 1,
          reportingTimezone: "UTC",
        },
      });
      return Promise.resolve(settingsResponse(2));
    };

    expect((await getSettings(invoker)).data.revision).toBe(1);
    expect(
      (
        await updateSettings(
          {
            expectedRevision: 1,
            reportingTimezone: "UTC",
            backgroundRefreshEnabled: false,
            refreshIntervalMinutes: 15,
            launchAtLogin: false,
            closeBehavior: "quit",
            notificationsEnabled: false,
            storeProjectPaths: false,
          },
          invoker,
        )
      ).data.revision,
    ).toBe(2);
  });

  it("wraps project-path retention transitions in the named request argument", async () => {
    const invoker: CommandInvoker = (command, request) => {
      expect(command).toBe(COMMAND_NAMES.settingsUpdateProjectPathRetention);
      expect(request).toEqual({
        request: { expectedRevision: 1, retainPaths: false },
      });
      return Promise.resolve({
        ok: true,
        data: {
          settings: settingsResponseData(2),
          clearedPaths: 3,
        },
        meta,
      });
    };

    const result = await updateProjectPathRetention(
      { expectedRevision: 1, retainPaths: false },
      invoker,
    );

    expect(result.data.clearedPaths).toBe(3);
    expect(result.data.settings.revision).toBe(2);
  });

  it("rejects invalid settings before transport", async () => {
    const invoker = vi.fn<CommandInvoker>();

    await expect(
      updateSettings(
        {
          expectedRevision: 1,
          reportingTimezone: "UTC",
          backgroundRefreshEnabled: true,
          refreshIntervalMinutes: 1,
          launchAtLogin: false,
          closeBehavior: "quit",
          notificationsEnabled: false,
          storeProjectPaths: false,
        },
        invoker,
      ),
    ).rejects.toBeInstanceOf(ZodError);
    expect(invoker).not.toHaveBeenCalled();
  });
});

describe("budget IPC queries", () => {
  it("invokes list, get, and create contracts", async () => {
    const invoker = vi.fn<CommandInvoker>((command, request) => {
      if (command === COMMAND_NAMES.budgetsList) {
        expect(request).toEqual({});
        return Promise.resolve({
          ok: true,
          data: { items: [budgetResponseData("1")] },
          meta,
        });
      }
      return Promise.resolve({
        ok: true,
        data: budgetResponseData("2"),
        meta,
      });
    });

    expect((await listBudgets(invoker)).data.items[0]?.revision).toBe("1");
    expect((await getBudget({ budgetId: "7" }, invoker)).data.id).toBe("7");
    expect(
      (await createBudget({ budget: tokenBudgetDefinition() }, invoker)).data
        .limit,
    ).toEqual({ kind: "tokens", value: "100000" });
    expect(invoker.mock.calls.map(([command]) => command)).toEqual([
      COMMAND_NAMES.budgetsList,
      COMMAND_NAMES.budgetsGet,
      COMMAND_NAMES.budgetsCreate,
    ]);
  });
});

describe("budget IPC mutations", () => {
  it("invokes update, disable, enable, and delete contracts", async () => {
    const invoker = vi.fn<CommandInvoker>((command, request) => {
      if (command === COMMAND_NAMES.budgetsDelete) {
        expect(request).toEqual({
          request: { budgetId: "7", expectedRevision: "4" },
        });
        return Promise.resolve({
          ok: true,
          data: { budgetId: "7" },
          meta,
        });
      }
      return Promise.resolve({
        ok: true,
        data: budgetResponseData("2"),
        meta,
      });
    });

    expect(
      (
        await updateBudget(
          {
            budgetId: "7",
            expectedRevision: "1",
            budget: tokenBudgetDefinition(),
          },
          invoker,
        )
      ).data.revision,
    ).toBe("2");
    expect(
      (await disableBudget({ budgetId: "7", expectedRevision: "2" }, invoker))
        .data.enabled,
    ).toBe(true);
    expect(
      (await enableBudget({ budgetId: "7", expectedRevision: "3" }, invoker))
        .data.revision,
    ).toBe("2");
    expect(
      (await deleteBudget({ budgetId: "7", expectedRevision: "4" }, invoker))
        .data.budgetId,
    ).toBe("7");
    expect(invoker.mock.calls.map(([command]) => command)).toEqual([
      COMMAND_NAMES.budgetsUpdate,
      COMMAND_NAMES.budgetsDisable,
      COMMAND_NAMES.budgetsEnable,
      COMMAND_NAMES.budgetsDelete,
    ]);
    expect(invoker.mock.calls[0]?.[1]).toEqual({
      request: {
        budgetId: "7",
        expectedRevision: "1",
        budget: tokenBudgetDefinition(),
      },
    });
  });
});

describe("budget IPC validation", () => {
  it("rejects malformed exact values and duplicate thresholds before transport", async () => {
    const invoker = vi.fn<CommandInvoker>();

    await expect(
      createBudget(
        {
          budget: {
            ...tokenBudgetDefinition(),
            limit: { kind: "tokens", value: "01" },
          },
        },
        invoker,
      ),
    ).rejects.toBeInstanceOf(ZodError);
    await expect(
      getBudget({ budgetId: "not-an-id" }, invoker),
    ).rejects.toBeInstanceOf(ZodError);
    await expect(
      updateBudget(
        {
          budgetId: "7",
          expectedRevision: "1",
          budget: {
            ...tokenBudgetDefinition(),
            thresholds: [
              { basisPoints: 8000, enabled: true },
              { basisPoints: 8000, enabled: false },
            ],
          },
        },
        invoker,
      ),
    ).rejects.toBeInstanceOf(ZodError);
    expect(invoker).not.toHaveBeenCalled();
  });
});

describe("budget IPC response validation", () => {
  it("rejects malformed budget responses at the boundary", async () => {
    const invoker: CommandInvoker = () =>
      Promise.resolve({
        ok: true,
        data: {
          ...budgetResponseData("1"),
          scope: { kind: "source", sourceId: "0" },
        },
        meta,
      });

    await expect(getBudget({ budgetId: "7" }, invoker)).rejects.toBeInstanceOf(
      ZodError,
    );
  });
});

describe("usage overview IPC", () => {
  it("invokes and validates the usage overview contract", async () => {
    const invoker: CommandInvoker = (command, request) => {
      expect(command).toBe(COMMAND_NAMES.usageGetOverview);
      expect(request).toEqual({
        request: {
          startDate: "2026-06-13",
          endDate: "2026-06-14",
          reportingTimezone: "UTC",
        },
      });
      return Promise.resolve(usageOverview());
    };

    const result = await getUsageOverview(
      {
        startDate: "2026-06-13",
        endDate: "2026-06-14",
        reportingTimezone: "UTC",
      },
      invoker,
    );

    expect(result.data.totalTokens).toBe("18446744073709551615");
    expect(result.data.cost.valuation).toBe("estimated");
    expect(result.data.cost.completeness).toBe("partial");
    expect(result.data.dataStatus).toBe("partial");
    expect(result.data.sources.map((source) => source.source)).toContain(
      "opencode",
    );
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

function tokenBudgetDefinition(): BudgetDefinition {
  return {
    name: "Monthly tokens",
    limit: { kind: "tokens", value: "100000" },
    period: "monthly",
    scope: { kind: "global" },
    enabled: true,
    thresholds: [
      { basisPoints: 8000, enabled: true },
      { basisPoints: 10000, enabled: true },
    ],
  };
}

function budgetResponseData(revision: string) {
  return {
    id: "7",
    revision,
    ...tokenBudgetDefinition(),
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
        backgroundRefreshEnabled: false,
        refreshIntervalMinutes: 15,
        launchAtLogin: false,
        closeBehavior: "quit",
        notificationsEnabled: false,
        storeProjectPaths: false,
        revision: 1,
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

function refreshState(): IpcResponse<unknown> {
  return {
    ok: true,
    data: {
      status: "succeeded",
      jobId: "refresh-1000-0",
      trigger: "manual",
      lastSuccessfulRefreshAt: "2026-06-15T00:00:00+00:00",
    },
    meta,
  };
}

function settingsResponse(revision: number): IpcResponse<unknown> {
  return {
    ok: true,
    data: settingsResponseData(revision),
    meta,
  };
}

function settingsResponseData(revision: number) {
  return {
    reportingTimezone: "UTC",
    backgroundRefreshEnabled: false,
    refreshIntervalMinutes: 15,
    launchAtLogin: false,
    closeBehavior: "quit" as const,
    notificationsEnabled: false,
    storeProjectPaths: false,
    revision,
  };
}

function usageOverview(): IpcResponse<UsageOverviewResponse> {
  return {
    ok: true,
    data: {
      period: {
        startDate: "2026-06-13",
        endDate: "2026-06-14",
        reportingTimezone: "UTC",
      },
      totalTokens: "18446744073709551615",
      activeDays: 2,
      cost: {
        amountMicros: "630000",
        currency: "USD",
        valuation: "estimated",
        completeness: "partial",
        unavailableDays: 1,
      },
      sources: [
        {
          source: "claude-code",
          totalTokens: "18446744073709551615",
          activeDays: 2,
          cost: {
            amountMicros: "630000",
            currency: "USD",
            valuation: "estimated",
            completeness: "partial",
            unavailableDays: 1,
          },
          hasPartialData: true,
        },
        {
          source: "opencode",
          totalTokens: "1200",
          activeDays: 1,
          cost: {
            amountMicros: null,
            currency: null,
            valuation: "unavailable",
            completeness: "unavailable",
            unavailableDays: 1,
          },
          hasPartialData: false,
        },
      ],
      models: [],
      asOf: "2026-06-15T07:30:00.000Z",
      lastSuccessfulRefreshAt: "2026-06-15T07:00:00.000Z",
      dataStatus: "partial",
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

describe("usage overview IPC validation", () => {
  it("rejects inconsistent overview cost and integer payloads", async () => {
    const malformed = usageOverview();
    if (malformed.ok) {
      malformed.data.totalTokens = "01";
      malformed.data.cost.valuation = "unavailable";
    }
    const invoker: CommandInvoker = () => Promise.resolve(malformed);

    await expect(
      getUsageOverview(
        {
          startDate: "2026-06-13",
          endDate: "2026-06-14",
          reportingTimezone: "UTC",
        },
        invoker,
      ),
    ).rejects.toBeInstanceOf(ZodError);
  });

  it("rejects invalid overview requests before transport", async () => {
    let invoked = false;
    const invoker: CommandInvoker = () => {
      invoked = true;
      return Promise.resolve(usageOverview());
    };

    await expect(
      getUsageOverview(
        {
          startDate: "13-06-2026",
          endDate: "2026-06-14",
          reportingTimezone: "UTC",
        },
        invoker,
      ),
    ).rejects.toBeInstanceOf(ZodError);
    expect(invoked).toBe(false);
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
