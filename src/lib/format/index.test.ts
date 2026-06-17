import { describe, expect, it } from "vitest";

import { formatCurrency, formatNumber } from ".";

describe("formatNumber", () => {
  it("formats exact unsigned integer strings without number coercion", () => {
    expect(formatNumber("18446744073709551615")).toBe(
      "18,446,744,073,709,551,615",
    );
  });

  it("rejects unsafe numbers", () => {
    expect(formatNumber(Number.MAX_SAFE_INTEGER + 1)).toBe("0");
  });
});

describe("formatCurrency", () => {
  it("formats micros exactly without floating point conversion", () => {
    expect(formatCurrency("123456789012345678", "USD")).toBe(
      "USD 123,456,789,012.345678",
    );
  });

  it("keeps at least two fractional digits", () => {
    expect(formatCurrency("3500000", "USD")).toBe("USD 3.50");
  });
});
