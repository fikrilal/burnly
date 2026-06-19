import { expect, test } from "@playwright/test";

const settings = {
  reportingTimezone: "UTC",
  backgroundRefreshEnabled: false,
  refreshIntervalMinutes: 15,
  launchAtLogin: false,
  closeBehavior: "quit",
  notificationsEnabled: false,
  storeProjectPaths: false,
  revision: 1,
} as const;

const meta = () => ({
  contractVersion: 1,
  requestId: crypto.randomUUID(),
  generatedAt: new Date().toISOString(),
});

test.describe("Desktop Evidence: overview states", () => {
  test("captures populated dashboard evidence", async ({ page }, testInfo) => {
    await installTauriMock(page, "populated");

    await page.goto("/");
    await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();
    await expect(page.getByText("1,500,000").first()).toBeVisible();

    await page.screenshot({
      path: `screenshots/evidence-${testInfo.project.name.toLowerCase()}-populated.png`,
      fullPage: true,
    });
  });

  test("captures empty state evidence", async ({ page }, testInfo) => {
    await installTauriMock(page, "empty");

    await page.goto("/");
    await expect(page.getByText("No data collected")).toBeVisible();

    await page.screenshot({
      path: `screenshots/evidence-${testInfo.project.name.toLowerCase()}-empty.png`,
      fullPage: true,
    });
  });

  test("captures error state evidence", async ({ page }, testInfo) => {
    await installTauriMock(page, "error");

    await page.goto("/");
    await expect(page.getByText("Failed to load overview data")).toBeVisible();
    await expect(
      page.getByText("Burnly could not load overview data. Try again."),
    ).toBeVisible();

    await page.screenshot({
      path: `screenshots/evidence-${testInfo.project.name.toLowerCase()}-error.png`,
      fullPage: true,
    });
  });

  test("refresh invalidation re-queries authoritative overview", async ({
    page,
  }) => {
    await installTauriMock(page, "populated");

    await page.goto("/");
    await expect(page.getByText("1,500,000").first()).toBeVisible();

    await page.getByRole("button", { name: "Refresh Now" }).click();

    await expect(page.getByText("Refresh running")).toBeVisible();
    await expect(page.getByText("2,000,000").first()).toBeVisible();
  });

  test("settings load and save through the dedicated contract", async ({
    page,
  }) => {
    await installTauriMock(page, "populated");

    await page.goto("/");
    await page.getByRole("button", { name: "Settings" }).click();
    const timezone = page.getByLabel("Reporting timezone");
    await expect(timezone).toHaveValue("UTC");
    await timezone.fill("Asia/Jakarta");
    await page.getByRole("button", { name: "Save settings" }).click();

    await expect(page.getByText("Settings saved.")).toBeVisible();
    await expect(timezone).toHaveValue("Asia/Jakarta");
  });

  test("project-path retention requires confirmation before deletion", async ({
    page,
  }) => {
    await installTauriMock(page, "populated");
    await page.goto("/");
    await page.getByRole("button", { name: "Settings" }).click();

    await page.getByRole("button", { name: "Enable" }).click();
    await expect(page.getByRole("button", { name: "Disable" })).toBeVisible();
    await page.getByRole("button", { name: "Disable" }).click();
    await expect(page.getByText("Remove stored project paths?")).toBeVisible();
    await page.getByRole("button", { name: "Remove paths" }).click();

    await expect(
      page.getByText("Removed 2 stored project paths."),
    ).toBeVisible();
  });

  test("captures populated budget interface evidence", async ({
    page,
  }, testInfo) => {
    await installTauriMock(page, "populated");

    await page.goto("/");
    await page.getByRole("button", { name: "Budgets" }).click();

    await expect(page.getByRole("heading", { name: "Budgets" })).toBeVisible();
    await expect(page.getByText("Monthly token cap")).toBeVisible();
    await expect(page.getByText("1,000,000 tokens")).toBeVisible();

    await page.screenshot({
      path: `screenshots/evidence-${testInfo.project.name.toLowerCase()}-budgets-populated.png`,
      fullPage: true,
    });
  });

  test("captures empty budget interface evidence", async ({
    page,
  }, testInfo) => {
    await installTauriMock(page, "empty");

    await page.goto("/");
    await page.getByRole("button", { name: "Budgets" }).click();

    await expect(page.getByText("No budgets yet")).toBeVisible();

    await page.screenshot({
      path: `screenshots/evidence-${testInfo.project.name.toLowerCase()}-budgets-empty.png`,
      fullPage: true,
    });
  });

  test("captures budget error state evidence", async ({ page }, testInfo) => {
    await installTauriMock(page, "error");

    await page.goto("/");
    await page.getByRole("button", { name: "Budgets" }).click();

    await expect(page.getByText("Budgets unavailable")).toBeVisible();
    await expect(page.getByText("Simulated budget error")).toBeVisible();

    await page.screenshot({
      path: `screenshots/evidence-${testInfo.project.name.toLowerCase()}-budgets-error.png`,
      fullPage: true,
    });
  });
});

async function installTauriMock(
  page: Parameters<Parameters<typeof test>[1]>[0]["page"],
  mode: "populated" | "empty" | "error",
) {
  await page.addInitScript((initialMode) => {
    let overviewMode = initialMode;
    let overviewTokens = "1500000";
    let nextEventId = 1;
    const callbacks = new Map<number, (event: unknown) => void>();
    const listeners = new Map<string, Set<number>>();
    const pageMeta = () => ({
      contractVersion: 1,
      requestId: crypto.randomUUID(),
      generatedAt: new Date().toISOString(),
    });

    const pageSettings = {
      reportingTimezone: "UTC",
      backgroundRefreshEnabled: false,
      refreshIntervalMinutes: 15,
      launchAtLogin: false,
      closeBehavior: "quit",
      notificationsEnabled: false,
      storeProjectPaths: false,
      revision: 1,
    };
    const pageBudgets = [
      {
        id: "7",
        revision: "1",
        name: "Monthly token cap",
        limit: { kind: "tokens", value: "1000000" },
        period: "monthly",
        scope: { kind: "global" },
        enabled: true,
        thresholds: [
          { basisPoints: 8000, enabled: true },
          { basisPoints: 10000, enabled: true },
        ],
      },
      {
        id: "8",
        revision: "2",
        name: "Weekly source cost",
        limit: { kind: "cost", amountMicros: "12500000", currency: "USD" },
        period: "weekly",
        scope: { kind: "source", sourceId: "2" },
        enabled: false,
        thresholds: [{ basisPoints: 9000, enabled: true }],
      },
    ];

    const pageBootstrapResponse = () => ({
      ok: true,
      meta: pageMeta(),
      data: {
        appVersion: "1.0.0",
        contractVersion: 1,
        database: { status: "ready", schemaVersion: 1 },
        settings: pageSettings,
        features: {
          usageOverview: true,
          collectorRefresh: true,
          budgets: true,
          settings: true,
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
        onboardingComplete: true,
      },
    });

    const pageCapabilitiesResponse = () => {
      const unavailableCapability = {
        supported: false,
        status: "not_implemented",
      };
      return {
        ok: true,
        meta: pageMeta(),
        data: {
          tray: { supported: true, status: "available" },
          launchAtLogin: unavailableCapability,
          nativeNotifications: {
            ...unavailableCapability,
            permission: "unknown",
          },
          updates: unavailableCapability,
          exportFormats: ["csv"],
          diagnostics: { desktopEvidence: true },
        },
      };
    };

    const pageRefreshResponse = (status: "idle" | "running") => ({
      ok: true,
      meta: pageMeta(),
      data: {
        status,
        lastSuccessfulRefreshAt: null,
        jobId: status === "running" ? "refresh-evidence-1" : null,
        trigger: status === "running" ? "manual" : null,
      },
    });

    const pageOverviewResponse = (
      totalTokens: string,
      dataStatus: "current" | "empty",
    ) => ({
      period: {
        startDate: "2026-05-16",
        endDate: "2026-06-15",
        reportingTimezone: "UTC",
      },
      totalTokens,
      activeDays: dataStatus === "empty" ? 0 : 14,
      cost: {
        amountMicros: dataStatus === "empty" ? null : "3500000",
        currency: dataStatus === "empty" ? null : "USD",
        valuation: dataStatus === "empty" ? "unavailable" : "estimated",
        completeness: dataStatus === "empty" ? "unavailable" : "partial",
        unavailableDays: dataStatus === "empty" ? 31 : 1,
      },
      sources:
        dataStatus === "empty"
          ? []
          : [
              {
                source: "claude-code",
                totalTokens,
                activeDays: 14,
                cost: {
                  amountMicros: "3500000",
                  currency: "USD",
                  valuation: "estimated",
                  completeness: "partial",
                  unavailableDays: 1,
                },
                hasPartialData: true,
              },
            ],
      models:
        dataStatus === "empty"
          ? []
          : [
              {
                name: "claude-sonnet-4",
                totalTokens,
                cost: {
                  amountMicros: "3500000",
                  currency: "USD",
                  valuation: "estimated",
                  completeness: "partial",
                  unavailableDays: 1,
                },
              },
            ],
      asOf: "2026-06-15T12:00:00Z",
      lastSuccessfulRefreshAt:
        dataStatus === "empty" ? null : "2026-06-15T10:00:00Z",
      dataStatus,
    });

    const pageBudgetProgressResponse = () => {
      if (overviewMode === "empty") {
        return {
          status: "no_budgets",
          reportingTimezone: "UTC",
          asOf: "2026-06-15T12:00:00Z",
          configuredBudgetCount: 0,
          enabledBudgetCount: 0,
          traySummary: null,
          items: [],
        };
      }

      return {
        status: "available",
        reportingTimezone: "UTC",
        asOf: "2026-06-15T12:00:00Z",
        configuredBudgetCount: 2,
        enabledBudgetCount: 1,
        traySummary: "Budget: Monthly token cap 75%",
        items: [
          {
            budgetId: "7",
            budgetName: "Monthly token cap",
            period: "monthly",
            periodStartDate: "2026-06-01",
            periodEndDate: "2026-06-30",
            metric: "tokens",
            state: "available",
            current: overviewTokens,
            limit: "2000000",
            currency: null,
            basisPoints: overviewTokens === "2000000" ? "10000" : "7500",
            exceeded: overviewTokens === "2000000",
            completeness: "complete",
            unavailableDays: 0,
          },
        ],
      };
    };

    const emit = (event: string, payload: Record<string, unknown>) => {
      const ids = listeners.get(event);
      if (!ids) return;

      for (const id of ids) {
        callbacks.get(id)?.({ event, id, payload });
      }
    };

    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener(event: string, eventId: number) {
        listeners.get(event)?.delete(eventId);
        callbacks.delete(eventId);
      },
    };

    window.__TAURI_INTERNALS__ = {
      transformCallback(callback: (event: unknown) => void) {
        const id = nextEventId;
        nextEventId += 1;
        callbacks.set(id, callback);
        return id;
      },
      unregisterCallback(id: number) {
        callbacks.delete(id);
      },
      invoke(command: string, args: Record<string, unknown>) {
        if (command === "plugin:event|listen") {
          const event = String(args.event);
          const handler = Number(args.handler);
          const eventIds = listeners.get(event) ?? new Set<number>();
          eventIds.add(handler);
          listeners.set(event, eventIds);
          return Promise.resolve(handler);
        }

        if (command === "plugin:event|unlisten") {
          const event = String(args.event);
          const eventId = Number(args.eventId);
          listeners.get(event)?.delete(eventId);
          callbacks.delete(eventId);
          return Promise.resolve(null);
        }

        if (command === "__burnly_contract_probe") {
          return Promise.resolve({
            ok: true,
            meta: pageMeta(),
            data: { status: "ok", contractVersion: 1 },
          });
        }

        if (command === "app_get_bootstrap") {
          return Promise.resolve(pageBootstrapResponse());
        }

        if (command === "app_get_capabilities") {
          return Promise.resolve(pageCapabilitiesResponse());
        }

        if (command === "settings_get") {
          return Promise.resolve({
            ok: true,
            meta: pageMeta(),
            data: pageSettings,
          });
        }

        if (command === "settings_update") {
          const request = args.request as typeof pageSettings & {
            expectedRevision: number;
          };
          Object.assign(pageSettings, request, {
            revision: request.expectedRevision + 1,
          });
          emit("burnly://v1/settings-changed", {});
          return Promise.resolve({
            ok: true,
            meta: pageMeta(),
            data: pageSettings,
          });
        }

        if (command === "settings_update_project_path_retention") {
          const request = args.request as {
            expectedRevision: number;
            retainPaths: boolean;
          };
          Object.assign(pageSettings, {
            storeProjectPaths: request.retainPaths,
            revision: request.expectedRevision + 1,
          });
          emit("burnly://v1/settings-changed", {});
          emit("burnly://v1/data-invalidated", { scope: "sessions" });
          return Promise.resolve({
            ok: true,
            meta: pageMeta(),
            data: {
              settings: pageSettings,
              clearedPaths: request.retainPaths ? 0 : 2,
            },
          });
        }

        if (command === "budgets_list") {
          if (overviewMode === "error") {
            return Promise.resolve({
              ok: false,
              meta: pageMeta(),
              error: {
                code: "budgets.storage_unavailable",
                message: "Simulated budget error",
                category: "persistence",
                retryable: true,
                details: null,
              },
            });
          }

          return Promise.resolve({
            ok: true,
            meta: pageMeta(),
            data: {
              items: overviewMode === "empty" ? [] : pageBudgets,
            },
          });
        }

        if (command === "budgets_get_progress") {
          if (overviewMode === "error") {
            return Promise.resolve({
              ok: false,
              meta: pageMeta(),
              error: {
                code: "budgets.progress_unavailable",
                message: "Simulated budget progress error",
                category: "persistence",
                retryable: true,
                details: null,
              },
            });
          }

          return Promise.resolve({
            ok: true,
            meta: pageMeta(),
            data: pageBudgetProgressResponse(),
          });
        }

        if (command === "refresh_get_state") {
          return Promise.resolve(pageRefreshResponse("idle"));
        }

        if (command === "refresh_request") {
          setTimeout(() => {
            emit("burnly://v1/refresh-progress", { status: "running" });
            overviewTokens = "2000000";
            overviewMode = "populated";
            emit("burnly://v1/data-invalidated", { scope: "usage" });
          }, 20);

          return Promise.resolve(pageRefreshResponse("running"));
        }

        if (command === "usage_get_overview") {
          if (overviewMode === "error") {
            return Promise.resolve({
              ok: false,
              meta: pageMeta(),
              error: {
                code: "overview.unavailable",
                message: "Simulated overview error",
                category: "unavailable",
                retryable: true,
                details: null,
              },
            });
          }

          return Promise.resolve({
            ok: true,
            meta: pageMeta(),
            data:
              overviewMode === "empty"
                ? pageOverviewResponse("0", "empty")
                : pageOverviewResponse(overviewTokens, "current"),
          });
        }

        return Promise.resolve({ ok: true, meta: pageMeta(), data: null });
      },
    };

    window.__TAURI_IPC__ = window.__TAURI_INTERNALS__.invoke;
  }, mode);
}

function bootstrapResponse() {
  return {
    ok: true,
    meta: meta(),
    data: {
      appVersion: "1.0.0",
      contractVersion: 1,
      database: { status: "ready", schemaVersion: 1 },
      settings,
      features: {
        usageOverview: true,
        collectorRefresh: true,
        budgets: true,
        settings: true,
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
      onboardingComplete: true,
    },
  };
}

function capabilitiesResponse() {
  const unavailableCapability = { supported: false, status: "not_implemented" };
  return {
    ok: true,
    meta: meta(),
    data: {
      tray: { supported: true, status: "available" },
      launchAtLogin: unavailableCapability,
      nativeNotifications: {
        ...unavailableCapability,
        permission: "unknown",
      },
      updates: unavailableCapability,
      exportFormats: ["csv"],
      diagnostics: { desktopEvidence: true },
    },
  };
}

function refreshResponse(status: "idle" | "running") {
  return {
    ok: true,
    meta: meta(),
    data: {
      status,
      lastSuccessfulRefreshAt: null,
      jobId: status === "running" ? "refresh-evidence-1" : null,
      trigger: status === "running" ? "manual" : null,
    },
  };
}

function overviewResponse(
  totalTokens: string,
  dataStatus: "current" | "empty",
) {
  return {
    period: {
      startDate: "2026-05-16",
      endDate: "2026-06-15",
      reportingTimezone: "UTC",
    },
    totalTokens,
    activeDays: dataStatus === "empty" ? 0 : 14,
    cost: {
      amountMicros: dataStatus === "empty" ? null : "3500000",
      currency: dataStatus === "empty" ? null : "USD",
      valuation: dataStatus === "empty" ? "unavailable" : "estimated",
      completeness: dataStatus === "empty" ? "unavailable" : "partial",
      unavailableDays: dataStatus === "empty" ? 31 : 1,
    },
    sources:
      dataStatus === "empty"
        ? []
        : [
            {
              source: "claude-code",
              totalTokens,
              activeDays: 14,
              cost: {
                amountMicros: "3500000",
                currency: "USD",
                valuation: "estimated",
                completeness: "partial",
                unavailableDays: 1,
              },
              hasPartialData: true,
            },
          ],
    models:
      dataStatus === "empty"
        ? []
        : [
            {
              name: "claude-sonnet-4",
              totalTokens,
              cost: {
                amountMicros: "3500000",
                currency: "USD",
                valuation: "estimated",
                completeness: "partial",
                unavailableDays: 1,
              },
            },
          ],
    asOf: "2026-06-15T12:00:00Z",
    lastSuccessfulRefreshAt:
      dataStatus === "empty" ? null : "2026-06-15T10:00:00Z",
    dataStatus,
  };
}
