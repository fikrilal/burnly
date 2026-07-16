import { describe, expect, it } from "vitest";

import { BurnlyClientError } from "../../ipc/errors";
import {
  accountErrorMessage,
  accountSessionErrorMessage,
} from "./account-errors";

describe("accountErrorMessage", () => {
  it("maps known BurnlyClientError codes", () => {
    const error = new BurnlyClientError({
      kind: "application",
      error: {
        code: "AUTH_USER_SUSPENDED",
        message: "raw",
        category: "unavailable",
        retryable: false,
        details: null,
      },
      requestId: "req",
      generatedAt: "2026-07-14T00:00:00.000Z",
    });
    expect(accountErrorMessage(error)).toBe(
      "This account is suspended and cannot sign in.",
    );
  });

  it("falls back safely", () => {
    expect(accountErrorMessage(undefined)).toBe(
      "Burnly could not update account status.",
    );
  });
});

describe("accountSessionErrorMessage", () => {
  it("prefers known codes over raw messages", () => {
    expect(
      accountSessionErrorMessage(
        "AUTH_DESKTOP_HANDOFF_INVALID",
        "should not leak",
      ),
    ).toBe("Sign-in expired or was invalid. Please try again.");
  });
});
