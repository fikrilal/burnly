import { test, expect } from "@playwright/test";

// This test mounts the frontend and mocks window.__TAURI_IPC__ to capture visual states
// without needing the full Rust backend binary compiled for testing UI.
test.describe("Desktop Evidence: UI States", () => {
  test.beforeEach(async ({ page }) => {
    // Intercept and mock Tauri IPC calls before page loads
    await page.addInitScript(() => {
      // Mock both __TAURI_INTERNALS__ and __TAURI_IPC__ for compatibility
      const mockInvoke = async (command: string, args: any) => {
        const meta = {
          contractVersion: 1,
          requestId: crypto.randomUUID(),
          generatedAt: new Date().toISOString(),
        };

        if (command === "__burnly_contract_probe") {
          return { ok: true, meta, data: { status: "ok", contractVersion: 1 } };
        }
        if (command === "usage_get_overview") {
          return {
            ok: true,
            meta,
            data: {
              period: {
                startDate: "2026-05-16",
                endDate: "2026-06-15",
                reportingTimezone: "UTC",
              },
              totalTokens: "1500000",
              activeDays: 14,
              cost: {
                amountMicros: "3500000",
                currency: "USD",
                valuation: "estimated",
                completeness: "partial",
                unavailableDays: 0,
              },
              sources: [
                {
                  source: "claude-code",
                  totalTokens: "1000000",
                  activeDays: 10,
                  cost: {
                    amountMicros: "2500000",
                    currency: "USD",
                    valuation: "available",
                    completeness: "complete",
                    unavailableDays: 0,
                  },
                  hasPartialData: false,
                },
              ],
              asOf: "2026-06-15T12:00:00Z",
              lastSuccessfulRefreshAt: "2026-06-15T10:00:00Z",
              dataStatus: "current",
            },
          };
        }
        if (command === "app_get_bootstrap") {
          return {
            ok: true,
            meta,
            data: {
              appVersion: "1.0.0",
              contractVersion: 1,
              database: { status: "ready", schemaVersion: 1 },
              settings: { reportingTimezone: "UTC" },
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
        if (command === "app_get_capabilities") {
          return {
            ok: true,
            meta,
            data: {
              tray: { supported: true, status: "not_implemented" },
              launchAtLogin: { supported: true, status: "not_implemented" },
              nativeNotifications: {
                supported: true,
                status: "not_implemented",
              },
              updates: { supported: true, status: "not_implemented" },
              exportFormats: ["csv"],
              diagnostics: { desktopEvidence: true },
            },
          };
        }
        if (command === "refresh_request" || command === "refresh_get_state") {
          return {
            ok: true,
            meta,
            data: {
              status: "idle",
              lastSuccessfulRefreshAt: null,
              jobId: null,
              trigger: "manual",
            },
          };
        }
        console.warn(`Unmocked IPC command: ${command}`);
        return { ok: true, meta, data: null };
      };

      window.__TAURI_INTERNALS__ = { invoke: mockInvoke };
      window.__TAURI_IPC__ = mockInvoke;
    });
  });

  test("captures populated dashboard evidence", async ({ page }, testInfo) => {
    await page.goto("/");
    await expect(page.locator("text=Overview").first()).toBeVisible();
    await expect(page.locator("text=1,500,000")).toBeVisible();

    // Ensure fonts and icons load before screenshot
    await page.waitForTimeout(500);

    await page.screenshot({
      path: `screenshots/evidence-${testInfo.project.name.toLowerCase()}-populated.png`,
      fullPage: true,
    });
  });

  test("captures empty state evidence", async ({ page }, testInfo) => {
    // Override the mock for empty state
    await page.addInitScript(() => {
      const mockInvoke = async (command: string) => {
        const meta = {
          contractVersion: 1,
          requestId: crypto.randomUUID(),
          generatedAt: new Date().toISOString(),
        };
        if (command === "__burnly_contract_probe") {
          return { ok: true, meta, data: { status: "ok", contractVersion: 1 } };
        }
        if (command === "usage_get_overview") {
          return {
            ok: true,
            meta,
            data: {
              period: {
                startDate: "2026-05-16",
                endDate: "2026-06-15",
                reportingTimezone: "UTC",
              },
              totalTokens: "0",
              activeDays: 0,
              cost: {
                amountMicros: "0",
                currency: "USD",
                valuation: "estimated",
                completeness: "complete",
                unavailableDays: 0,
              },
              sources: [],
              asOf: "2026-06-15T12:00:00Z",
              lastSuccessfulRefreshAt: null,
              dataStatus: "empty",
            },
          };
        }
        if (command === "app_get_bootstrap") {
          return {
            ok: true,
            meta,
            data: {
              appVersion: "1.0.0",
              contractVersion: 1,
              database: { status: "ready", schemaVersion: 1 },
              settings: { reportingTimezone: "UTC" },
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
        if (command === "app_get_capabilities") {
          return {
            ok: true,
            meta,
            data: {
              tray: { supported: true, status: "not_implemented" },
              launchAtLogin: { supported: true, status: "not_implemented" },
              nativeNotifications: {
                supported: true,
                status: "not_implemented",
              },
              updates: { supported: true, status: "not_implemented" },
              exportFormats: ["csv"],
              diagnostics: { desktopEvidence: true },
            },
          };
        }
        if (command === "refresh_request" || command === "refresh_get_state") {
          return {
            ok: true,
            meta,
            data: {
              status: "idle",
              lastSuccessfulRefreshAt: null,
              jobId: null,
              trigger: "manual",
            },
          };
        }
        return { ok: true, meta, data: null };
      };
      window.__TAURI_INTERNALS__ = { invoke: mockInvoke };
    });

    await page.goto("/");
    await expect(page.locator("text=No data collected")).toBeVisible();

    await page.screenshot({
      path: `screenshots/evidence-${testInfo.project.name.toLowerCase()}-empty.png`,
      fullPage: true,
    });
  });

  test("captures error state evidence", async ({ page }, testInfo) => {
    await page.addInitScript(() => {
      const mockInvoke = async (command: string) => {
        if (command === "__burnly_contract_probe") {
          return {
            ok: true,
            meta: {
              contractVersion: 1,
              requestId: crypto.randomUUID(),
              generatedAt: new Date().toISOString(),
            },
            data: { status: "ok", contractVersion: 1 },
          };
        }
        if (command === "usage_get_overview") {
          return {
            ok: false,
            meta: {
              contractVersion: 1,
              requestId: crypto.randomUUID(),
              generatedAt: new Date().toISOString(),
            },
            error: {
              code: "network.error",
              message: "Simulated network error",
              category: "unavailable",
              retryable: true,
              details: null,
            },
          };
        }
        if (command === "app_get_bootstrap") {
          return {
            ok: true,
            meta: {
              contractVersion: 1,
              requestId: crypto.randomUUID(),
              generatedAt: new Date().toISOString(),
            },
            data: {
              appVersion: "1.0.0",
              contractVersion: 1,
              database: { status: "ready", schemaVersion: 1 },
              settings: { reportingTimezone: "UTC" },
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
        if (command === "app_get_capabilities") {
          return {
            ok: true,
            meta: {
              contractVersion: 1,
              requestId: crypto.randomUUID(),
              generatedAt: new Date().toISOString(),
            },
            data: {
              tray: { supported: true, status: "not_implemented" },
              launchAtLogin: { supported: true, status: "not_implemented" },
              nativeNotifications: {
                supported: true,
                status: "not_implemented",
              },
              updates: { supported: true, status: "not_implemented" },
              exportFormats: ["csv"],
              diagnostics: { desktopEvidence: true },
            },
          };
        }
        if (command === "refresh_request" || command === "refresh_get_state") {
          return {
            ok: true,
            meta: {
              contractVersion: 1,
              requestId: crypto.randomUUID(),
              generatedAt: new Date().toISOString(),
            },
            data: {
              status: "idle",
              lastSuccessfulRefreshAt: null,
              jobId: null,
              trigger: "manual",
            },
          };
        }
        return {
          ok: true,
          meta: {
            contractVersion: 1,
            requestId: crypto.randomUUID(),
            generatedAt: new Date().toISOString(),
          },
          data: null,
        };
      };
      window.__TAURI_INTERNALS__ = { invoke: mockInvoke };
    });

    await page.goto("/");
    await expect(
      page.locator("text=Failed to load overview data"),
    ).toBeVisible();

    await page.screenshot({
      path: `screenshots/evidence-${testInfo.project.name.toLowerCase()}-error.png`,
      fullPage: true,
    });
  });
});
