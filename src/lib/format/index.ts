export const formatLayerName = "format";

const uintPattern = /^(0|[1-9][0-9]*)$/;

export function formatNumber(value: number | string | bigint): string {
  const digits = integerDigits(value);
  if (!digits) return "0";

  return groupDigits(digits);
}

export function formatCurrency(
  micros: string | null,
  currencyCode: string | null,
): string {
  if (!micros || !currencyCode) return "---";

  if (!uintPattern.test(micros)) return "---";

  const amountMicros = BigInt(micros);
  const whole = amountMicros / 1_000_000n;
  const fraction = amountMicros % 1_000_000n;
  const fractionText = formatMicrosFraction(fraction);

  return `${currencyCode} ${groupDigits(whole.toString())}.${fractionText}`;
}

export function formatDateTime(isoString: string | null): string {
  if (!isoString) return "Never";

  const date = new Date(isoString);
  if (Number.isNaN(date.getTime())) return "Invalid date";

  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

function integerDigits(value: number | string | bigint): string | null {
  if (typeof value === "bigint") {
    return value < 0n ? null : value.toString();
  }

  if (typeof value === "number") {
    if (!Number.isSafeInteger(value) || value < 0) return null;
    return String(value);
  }

  return uintPattern.test(value) ? value : null;
}

function groupDigits(digits: string): string {
  return digits.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

function formatMicrosFraction(fraction: bigint): string {
  const padded = fraction.toString().padStart(6, "0");
  const trimmed = padded.replace(/0+$/, "");
  return trimmed.padEnd(2, "0");
}
