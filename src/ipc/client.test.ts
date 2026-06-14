import { describe, expect, it } from "vitest";
import { ZodError } from "zod";

import {
  getContractProbe,
  invokeCommand,
  validateInt64String,
  validateUint64String,
} from "./client";
import {
  COMMAND_NAMES,
  CONTRACT_VERSION,
  type CommandInvoker,
} from "./generated/contracts";

const meta = {
  contractVersion: CONTRACT_VERSION,
  requestId: "018f5f4d-7758-7bb2-9d9b-6d7f22c4a901",
  generatedAt: "2026-06-14T07:30:00.000Z",
} as const;

describe("IPC command responses", () => {
  it("unwraps successful command envelopes with metadata", async () => {
    const invoker: CommandInvoker = () =>
      Promise.resolve({
        ok: true,
        data: {
          status: "ok",
          contractVersion: CONTRACT_VERSION,
        },
        meta,
      });

    const result = await getContractProbe(invoker);

    expect(result.data.status).toBe("ok");
    expect(result.meta.requestId).toBe(meta.requestId);
  });

  it("maps application error envelopes to typed client errors", async () => {
    const invoker: CommandInvoker = () =>
      Promise.resolve({
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
      });

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
