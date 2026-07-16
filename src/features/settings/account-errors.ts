import { BurnlyClientError } from "../../ipc/errors";

const ACCOUNT_ERROR_COPY: Record<string, string> = {
  AUTH_DESKTOP_HANDOFF_INVALID:
    "Sign-in expired or was invalid. Please try again.",
  AUTH_USER_SUSPENDED: "This account is suspended and cannot sign in.",
  RATE_LIMITED: "Too many sign-in attempts. Please wait and try again.",
  UNAUTHORIZED: "Your session is no longer valid. Please sign in again.",
  "account.state_mismatch": "Sign-in could not be verified. Please try again.",
  "account.login_timeout": "Sign-in timed out. Please try again.",
  "account.invalid_callback": "Burnly received an invalid sign-in callback.",
  "account.login_unavailable":
    "Sign-in is unavailable in this build or configuration.",
  "account.storage_failed":
    "Burnly could not save the signed-in session on this device.",
  "account.exchange_failed": "Sign-in failed. Please try again.",
  "account.open_browser_failed":
    "Burnly could not open the system browser to sign in.",
  "account.callback_bind_failed":
    "Burnly could not listen for the sign-in callback. Is the port already in use?",
  "account.logout_failed": "Burnly could not sign out of this device.",
};

/**
 * User-visible account error text. Prefers known codes; never invents secrets.
 */
export function accountErrorMessage(
  error: unknown,
  fallback = "Burnly could not update account status.",
): string {
  if (error instanceof BurnlyClientError) {
    const known = ACCOUNT_ERROR_COPY[error.code];
    if (known) return known;
    if (error.message.trim().length > 0) return error.message;
    return fallback;
  }
  if (error instanceof Error && error.message.trim().length > 0) {
    return error.message;
  }
  return fallback;
}

export function accountSessionErrorMessage(
  code: string | null | undefined,
  message: string | null | undefined,
  fallback = "Sign-in failed. Please try again.",
): string {
  if (code && ACCOUNT_ERROR_COPY[code]) {
    return ACCOUNT_ERROR_COPY[code];
  }
  if (message && message.trim().length > 0) {
    return message;
  }
  return fallback;
}
