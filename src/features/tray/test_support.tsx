import { render } from "@testing-library/react";
import { vi } from "vitest";

import { TrayPanel } from "./TrayPanel";
import {
  getAccountSession,
  getDiagnosticsHealth,
  type CommandResult,
} from "../../ipc/client";
import { subscribeToEvent } from "../../ipc/events";
import type {
  AccountSessionResponse,
  AppCapabilitiesResponse,
  SettingsResponse,
  TraySummaryResponse,
  UpdateStatusResponse,
} from "../../ipc/generated/contracts";
import { ThemeProvider } from "../../lib/theme";
import { createTestQueryWrapper } from "../../test/query";

export const responseMeta = {
  requestId: "1",
  contractVersion: 1,
  generatedAt: "2026-06-25T00:00:00Z",
} as const;

export const summary: TraySummaryResponse = {
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
  asOf: "2026-06-25T12:00:00Z",
  lastSuccessfulRefreshAt: "2026-06-25T11:55:00Z",
  dataStatus: "current",
};

export const capabilities: AppCapabilitiesResponse = {
  tray: { supported: true, status: "available" },
  launchAtLogin: { supported: true, status: "available" },
  update: { supported: false, status: "not_implemented" },
  exportFormats: ["csv"],
  diagnostics: {
    desktopEvidence: true,
    sendReport: { supported: false, status: "not_implemented" },
  },
};

export function resetTrayPanelMocks() {
  vi.clearAllMocks();
  vi.mocked(subscribeToEvent).mockResolvedValue(() => {
    /* no-op */
  });
  vi.mocked(getDiagnosticsHealth).mockResolvedValue(diagnosticsHealthResult());
  vi.mocked(getAccountSession).mockResolvedValue(accountSessionResult());
}

export function traySummaryResult(
  data: TraySummaryResponse = summary,
): CommandResult<TraySummaryResponse> {
  return { data, meta: responseMeta };
}

export function diagnosticsHealthResult(
  overrides: Partial<{
    status: "ok" | "warning" | "error";
    reasons: { code: string; message: string }[];
    generatedAt: string;
  }> = {},
) {
  return {
    data: {
      status: "ok" as const,
      reasons: [],
      generatedAt: "2026-06-25T00:00:00Z",
      ...overrides,
    },
    meta: responseMeta,
  };
}

export function renderTrayPanel(
  overrides: Partial<{
    capabilities: AppCapabilitiesResponse;
    appVersion: string;
    reportingTimezone: string;
  }> = {},
) {
  render(
    <TrayPanel
      reportingTimezone={overrides.reportingTimezone ?? "Asia/Jakarta"}
      appVersion={overrides.appVersion ?? "0.1.0"}
      capabilities={overrides.capabilities ?? capabilities}
    />,
    {
      wrapper: createTestWrapper(),
    },
  );
}

export function settingsResult(
  overrides: Partial<SettingsResponse> = {},
): CommandResult<SettingsResponse> {
  return {
    data: {
      launchAtLogin: false,
      closeBehavior: "hide",
      revision: 1,
      ...overrides,
    },
    meta: responseMeta,
  };
}

export function accountSessionResult(
  overrides: Partial<AccountSessionResponse> = {},
): CommandResult<AccountSessionResponse> {
  return {
    data: {
      status: "signed_out",
      email: null,
      userId: null,
      ...overrides,
    },
    meta: responseMeta,
  };
}

export function updateResult(
  overrides: Partial<UpdateStatusResponse> = {},
): CommandResult<UpdateStatusResponse> {
  return {
    data: {
      status: "idle",
      availableVersion: null,
      downloadedVersion: null,
      lastCheckedAt: null,
      error: null,
      ...overrides,
    },
    meta: responseMeta,
  };
}

export function longModelList(): TraySummaryResponse["models"] {
  return Array.from({ length: 12 }, (_, index) => ({
    modelName: `Model ${index + 1}`,
    agentLabel: "Agent",
    totalTokens: `${1000 + index}`,
    trend: null,
  }));
}

function createTestWrapper() {
  const QueryWrapper = createTestQueryWrapper();
  return ({ children }: { children: React.ReactNode }) => (
    <ThemeProvider>
      <QueryWrapper>{children}</QueryWrapper>
    </ThemeProvider>
  );
}
