export function userSafeErrorMessage(
  error: unknown,
  fallback = "Burnly could not load tray summary data.",
): string {
  return error instanceof Error ? error.message : fallback;
}
