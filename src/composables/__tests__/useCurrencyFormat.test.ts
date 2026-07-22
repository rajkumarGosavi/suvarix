import { beforeEach, describe, expect, it } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";
import { useUiStore } from "@/stores/ui";

describe("useCurrencyFormat", () => {
  let formatINR: (v: number) => string;
  let formatChange: (v: number) => string;
  let formatCompact: (v: number) => string;
  let formatPercent: (v: number) => string;

  beforeEach(() => {
    setActivePinia(createPinia());
    // Amounts are hidden by default; reveal them for the formatting assertions.
    useUiStore().hideAmounts = false;
    ({ formatINR, formatChange, formatCompact, formatPercent } = useCurrencyFormat());
  });

  describe("formatCompact", () => {
    it("uses Cr at and above the 1 crore threshold", () => {
      expect(formatCompact(1e7)).toBe("₹1.00Cr");
      expect(formatCompact(2.5e7)).toBe("₹2.50Cr");
    });

    it("uses L between 1 lakh (inclusive) and 1 crore (exclusive)", () => {
      expect(formatCompact(1e5)).toBe("₹1.00L");
      expect(formatCompact(9999999)).toBe("₹100.00L");
    });

    it("falls back to standard INR formatting below 1 lakh", () => {
      expect(formatCompact(99999)).toBe(formatINR(99999));
      expect(formatCompact(99999)).not.toContain("L");
      expect(formatCompact(99999)).not.toContain("Cr");
    });

    it("handles negative values with the same thresholds", () => {
      expect(formatCompact(-1.5e7)).toBe("₹-1.50Cr");
      expect(formatCompact(-2.5e5)).toBe("₹-2.50L");
    });
  });

  describe("formatChange", () => {
    it("prefixes a + sign for non-negative values", () => {
      expect(formatChange(0)).toBe(`+${formatINR(0)}`);
      expect(formatChange(500)).toBe(`+${formatINR(500)}`);
    });

    it("does not double up the sign for negative values", () => {
      const result = formatChange(-500);
      expect(result).toBe(formatINR(-500));
      expect(result.startsWith("+")).toBe(false);
    });
  });

  describe("formatPercent", () => {
    it("divides the input by 100 before formatting as a percentage", () => {
      const result = formatPercent(12.34);
      expect(result).toContain("12.34");
      expect(result).toContain("%");
    });

    it("handles zero and negative percentages", () => {
      expect(formatPercent(0)).toContain("0.00");
      expect(formatPercent(-5)).toContain("-5.00");
    });
  });

  describe("privacy masking", () => {
    it("masks every amount formatter when hideAmounts is on", () => {
      useUiStore().hideAmounts = true;
      const f = useCurrencyFormat();
      expect(f.formatINR(1234)).not.toContain("1,234");
      expect(f.formatINRDecimal(1234)).not.toContain("1,234");
      expect(f.formatCompact(1e7)).not.toContain("Cr");
      expect(f.formatChange(500)).not.toContain("500");
      expect(f.formatPercent(12.34)).not.toContain("12.34");
    });
  });
});
