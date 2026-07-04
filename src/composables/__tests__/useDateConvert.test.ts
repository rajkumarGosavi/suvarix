import { describe, expect, it } from "vitest";
import { dateToStr, strToDate } from "@/composables/useDateConvert";

describe("useDateConvert", () => {
  describe("strToDate", () => {
    it("returns null for null/undefined/empty input", () => {
      expect(strToDate(null)).toBeNull();
      expect(strToDate(undefined)).toBeNull();
      expect(strToDate("")).toBeNull();
    });

    it("parses a YYYY-MM-DD string into a local Date at midnight", () => {
      const d = strToDate("2024-03-15");
      expect(d).not.toBeNull();
      expect(d!.getFullYear()).toBe(2024);
      expect(d!.getMonth()).toBe(2); // 0-indexed: March
      expect(d!.getDate()).toBe(15);
      expect(d!.getHours()).toBe(0);
    });

    it("does not shift the date across a UTC day boundary", () => {
      // Regression case for the classic "new Date('YYYY-MM-DD') parses as UTC
      // midnight, which displays as the previous day in negative-UTC-offset
      // timezones" bug this composable exists to avoid.
      const d = strToDate("2024-01-01");
      expect(d!.getFullYear()).toBe(2024);
      expect(d!.getMonth()).toBe(0);
      expect(d!.getDate()).toBe(1);
    });
  });

  describe("dateToStr", () => {
    it("returns null for null/undefined input", () => {
      expect(dateToStr(null)).toBeNull();
      expect(dateToStr(undefined)).toBeNull();
    });

    it("formats a Date as YYYY-MM-DD with zero-padding", () => {
      expect(dateToStr(new Date(2024, 2, 15))).toBe("2024-03-15");
      expect(dateToStr(new Date(2024, 0, 5))).toBe("2024-01-05");
    });
  });

  describe("round trip", () => {
    it("strToDate and dateToStr are inverses", () => {
      const original = "2024-11-07";
      expect(dateToStr(strToDate(original))).toBe(original);
    });
  });
});
