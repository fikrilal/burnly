export const formatLayerName = "format";

export function formatNumber(value: number | string): string {
  const num = typeof value === "string" ? parseInt(value, 10) : value;
  if (Number.isNaN(num)) return "0";
  return new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 }).format(
    num,
  );
}

export function formatCurrency(
  micros: string | null,
  currencyCode: string | null,
): string {
  if (!micros || !currencyCode) return "---";

  const numMicros = parseInt(micros, 10);
  if (Number.isNaN(numMicros)) return "---";

  const amount = numMicros / 1_000_000;
  return new Intl.NumberFormat(undefined, {
    style: "currency",
    currency: currencyCode,
    minimumFractionDigits: 2,
    maximumFractionDigits: 4,
  }).format(amount);
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
