import type {
  ErrorCategory,
  FieldError,
  IpcError,
} from "./generated/contracts";

export type ClientErrorKind = "application" | "transport";

export class BurnlyClientError extends Error {
  readonly kind: ClientErrorKind;
  readonly code: string;
  readonly category: ErrorCategory;
  readonly retryable: boolean;
  readonly requestId: string;
  readonly generatedAt: string;
  readonly fieldErrors: readonly FieldError[];
  readonly details: null;
  readonly causeValue: unknown;

  constructor(input: {
    kind: ClientErrorKind;
    error: IpcError;
    requestId: string;
    generatedAt: string;
    cause?: unknown;
  }) {
    super(input.error.message);
    this.name = "BurnlyClientError";
    this.kind = input.kind;
    this.code = input.error.code;
    this.category = input.error.category;
    this.retryable = input.error.retryable;
    this.requestId = input.requestId;
    this.generatedAt = input.generatedAt;
    this.fieldErrors = input.error.fieldErrors ?? [];
    this.details = input.error.details;
    this.causeValue = input.cause;
  }
}
