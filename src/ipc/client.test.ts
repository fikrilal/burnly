import { describe, expect, it } from "vitest";

import {
  getAppBootstrap,
  getAppCapabilities,
  copyDiagnosticsReport,
  exportDiagnosticsReport,
  getDiagnosticsHealth,
  hideTrayPanel,
  openExternalUrl,
  getTraySummary,
  probeContract,
  validateInt64String,
  validateUint64String,
} from "./client";
import {
  COMMAND_NAMES,
  CONTRACT_VERSION,
  type CommandInvoker,
  type IpcResponse,
  type TraySummaryResponse,
} from "./generated/contracts";

const meta = {
  contractVersion: CONTRACT_VERSION,
  requestId: "018f5f4d-7758-7bb2-9d9b-6d7f22c4a901",
  generatedAt: "2026-06-14T07:30:00.000Z",
} as const;

describe("IPC command responses", () => {
  it("unwraps successful command envelopes with metadata", async () => {
    const invoker: CommandInvoker = () => Promise.resolve(contractProbe());

    const result = await probeContract(invoker);

    expect(result.data.status).toBe("ok");
    expect(result.meta.requestId).toBe(meta.requestId);
  });

  it("validates bootstrap data from the desktop runtime", async () => {
    const invoker: CommandInvoker = () => Promise.resolve(bootstrap());

    const result = await getAppBootstrap(invoker);

    expect(result.data.database.status).toBe("ready");
    expect(result.data.settings.closeBehavior).toBe("quit");
    expect(result.data.sources.status).toBe("not_configured");
  });

  it("validates desktop capability data from the desktop runtime", async () => {
    const invoker: CommandInvoker = () => Promise.resolve(capabilities());

    const result = await getAppCapabilities(invoker);

    expect(result.data.tray.status).toBe("not_implemented");
    expect(result.data.exportFormats).toEqual([]);
    expect(result.data.diagnostics.desktopEvidence).toBe(true);
    expect(result.data.diagnostics.sendReport.supported).toBe(false);
  });

  it("hides the tray panel through the dedicated app command", async () => {
    const invoker: CommandInvoker = (command, request) => {
      expect(command).toBe(COMMAND_NAMES.appHideTrayPanel);
      expect(request).toEqual({});
      return Promise.resolve({
        ok: true,
        data: { status: "hidden" },
        meta,
      });
    };

    const result = await hideTrayPanel(invoker);

    expect(result.data.status).toBe("hidden");
  });

  it("opens external URLs through the dedicated app command", async () => {
    const invoker: CommandInvoker = (command, request) => {
      expect(command).toBe(COMMAND_NAMES.appOpenExternalUrl);
      expect(request).toEqual({
        request: {
          url: "https://github.com/fikrilal/burnly/issues",
        },
      });
      return Promise.resolve({
        ok: true,
        data: { status: "opened" },
        meta,
      });
    };

    const result = await openExternalUrl(
      "https://github.com/fikrilal/burnly/issues",
      invoker,
    );

    expect(result.data.status).toBe("opened");
  });

  it("validates diagnostics health responses", async () => {
    const invoker: CommandInvoker = (command, request) => {
      expect(command).toBe(COMMAND_NAMES.diagnosticsGetHealth);
      expect(request).toEqual({});
      return Promise.resolve({
        ok: true,
        data: {
          status: "warning",
          reasons: [
            {
              code: "diagnostics.sources_failed",
              message: "Some sources failed during the last refresh.",
            },
          ],
          generatedAt: "2026-06-25T07:30:00.000Z",
        },
        meta,
      });
    };

    const result = await getDiagnosticsHealth(invoker);

    expect(result.data.status).toBe("warning");
    expect(result.data.reasons[0]?.code).toBe("diagnostics.sources_failed");
  });

  it("validates diagnostics export and copy responses", async () => {
    const exportInvoker: CommandInvoker = (command, request) => {
      expect(command).toBe(COMMAND_NAMES.diagnosticsExportReport);
      expect(request).toEqual({});
      return Promise.resolve({
        ok: true,
        data: { status: "exported" },
        meta,
      });
    };
    const copyInvoker: CommandInvoker = (command, request) => {
      expect(command).toBe(COMMAND_NAMES.diagnosticsCopyReport);
      expect(request).toEqual({});
      return Promise.resolve({
        ok: true,
        data: { status: "copied" },
        meta,
      });
    };

    await expect(exportDiagnosticsReport(exportInvoker)).resolves.toMatchObject(
      {
        data: { status: "exported" },
      },
    );
    await expect(copyDiagnosticsReport(copyInvoker)).resolves.toMatchObject({
      data: { status: "copied" },
    });
  });

  it("validates tray summary from the desktop runtime", async () => {
    const invoker: CommandInvoker = (command, request) => {
      expect(command).toBe(COMMAND_NAMES.usageGetTraySummary);
      expect(request).toEqual({
        request: { reportingTimezone: "Asia/Jakarta" },
      });
      return Promise.resolve(traySummary());
    };

    const result = await getTraySummary(
      { reportingTimezone: "Asia/Jakarta" },
      invoker,
    );

    expect(result.data.today.totalTokens).toBe("42180");
    expect(result.data.models[0]?.modelName).toBe("GPT-5.1");
  });
});

function contractProbe(): IpcResponse<{
  status: string;
  contractVersion: number;
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

function bootstrap(): IpcResponse<AppBootstrapResponse> {
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
        launchAtLogin: false,
        closeBehavior: "quit",
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

function capabilities(): IpcResponse<AppCapabilitiesResponse> {
  const capability = {
    supported: false,
    status: "not_implemented",
  } as const;

  return {
    ok: true,
    data: {
      tray: capability,
      launchAtLogin: capability,
      update: capability,
      exportFormats: [],
      diagnostics: {
        desktopEvidence: true,
        sendReport: capability,
      },
    },
    meta,
  };
}

function traySummary(): IpcResponse<TraySummaryResponse> {
  return {
    ok: true,
    data: {
      today: {
        startDate: "2026-06-25",
        endDate: "2026-06-25",
        totalTokens: "42180",
      },
      week: {
        startDate: "2026-06-22",
        endDate: "2026-06-28",
        totalTokens: "183240",
      },
      month: {
        startDate: "2026-06-01",
        endDate: "2026-06-30",
        totalTokens: "612900",
      },
      models: [
        {
          modelName: "GPT-5.1",
          agentLabel: "Codex",
          totalTokens: "25000",
          trend: {
            direction: "increased",
            basisPoints: 850,
          },
        },
        {
          modelName: "Claude Sonnet",
          agentLabel: "Claude Code",
          totalTokens: "12000",
          trend: null,
        },
      ],
      asOf: "2026-06-25T07:30:00.000Z",
      lastSuccessfulRefreshAt: "2026-06-25T07:25:00.000Z",
      dataStatus: "current",
    },
    meta,
  };
}

describe("IPC transport and validation failures", () => {
  it("maps invocation rejection to a synthetic transport error", async () => {
    const cause = new Error("command not registered");
    const invoker: CommandInvoker = () => Promise.reject(cause);

    await expect(invoker(COMMAND_NAMES.appGetBootstrap, {})).rejects.toBe(
      cause,
    );
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
import type {
  AppBootstrapResponse,
  AppCapabilitiesResponse,
} from "./generated/contracts";
