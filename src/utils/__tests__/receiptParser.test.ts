import { describe, it, expect } from "vitest";
import { parseReceipt } from "../receiptParser";

function ymd(d: Date | null): string | null {
    if (!d) return null;
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

describe("parseReceipt", () => {
    it("parses a supermarket slip — skips SUB TOTAL / SAVINGS / tendered cash, ignores GSTIN and phone", () => {
        const text = [
            "AVENUE SUPERMARTS LTD",
            "DMART JUNCTION",
            "GSTIN: 27AABCA1234B1Z5",
            "PH: 9876543210",
            "TAX INVOICE",
            "BILL NO: 12345 DT: 05/07/2026",
            "RICE 5KG 425.00",
            "OIL 1L 180.00",
            "SUB TOTAL 1,245.00",
            "TOTAL SAVINGS 155.00",
            "TOTAL 1,090.00",
            "CASH TENDERED 1,100.00",
        ].join("\n");
        const p = parseReceipt(text);
        expect(p.merchant).toBe("AVENUE SUPERMARTS LTD");
        expect(p.amount).toBe(1090);
        expect(ymd(p.date)).toBe("2026-07-05");
        expect(p.category).toBe("Shopping"); // DMART rule
    });

    it("prefers GRAND TOTAL over SUB TOTAL and ignores GST percentages", () => {
        const text = [
            "Hotel Sagar Ratna",
            "Veg Restaurant",
            "Date: 12-07-2026 Time: 21:45",
            "Paneer Tikka 380.00",
            "SUB TOTAL 840.00",
            "CGST 2.5% 21.00",
            "SGST 2.5% 21.00",
            "GRAND TOTAL 882.00",
        ].join("\n");
        const p = parseReceipt(text);
        expect(p.merchant).toBe("Hotel Sagar Ratna");
        expect(p.amount).toBe(882);
        expect(ymd(p.date)).toBe("2026-07-12");
        expect(p.category).toBe("Food"); // RESTAURANT rule
    });

    it("reads the value from the next line when OCR splits label and amount", () => {
        const text = [
            "ANAND PETROL PUMP",
            "HPCL DEALER",
            "05-Jul-2026",
            "PETROL 2.5L",
            "TOTAL",
            "500.00",
        ].join("\n");
        const p = parseReceipt(text);
        expect(p.merchant).toBe("ANAND PETROL PUMP");
        expect(p.amount).toBe(500);
        expect(ymd(p.date)).toBe("2026-07-05");
        expect(p.category).toBe("Travel"); // PETROL/HPCL rule
    });

    it("handles Indian digit grouping and NET PAYABLE, ISO dates, invoice-line merchant skip", () => {
        const text = [
            "Reliance Digital",
            "Invoice No: INV-889900",
            "Date: 2026-07-10",
            "Laptop 1,05,000.00",
            "GST 3,540.00",
            "NET PAYABLE 1,08,540.00",
        ].join("\n");
        const p = parseReceipt(text);
        expect(p.merchant).toBe("Reliance Digital");
        expect(p.amount).toBe(108540);
        expect(ymd(p.date)).toBe("2026-07-10");
        expect(p.category).toBe("Shopping"); // RELIANCE rule
    });

    it("falls back to the largest money value when no total keyword exists", () => {
        const text = [
            "Sharma General Store",
            "Ph: 022-2345678",
            "Milk 60.00",
            "Bread 45.00",
            "Eggs 120.00",
        ].join("\n");
        const p = parseReceipt(text);
        expect(p.merchant).toBe("Sharma General Store");
        expect(p.amount).toBe(120);
        expect(p.date).toBeNull();
        expect(p.category).toBe("Other");
    });

    it("rejects future dates", () => {
        const text = ["Cafe Coffee Day", "Date: 25/12/2027", "TOTAL 350.00"].join("\n");
        const p = parseReceipt(text);
        expect(p.date).toBeNull();
        expect(p.amount).toBe(350);
        expect(p.category).toBe("Food"); // CAFE rule
    });

    it("skips a TAX INVOICE header line when picking the merchant", () => {
        const text = [
            "TAX INVOICE",
            "Apollo Pharmacy",
            "GSTIN 33AAACA1234A1Z2",
            "Date: 01/07/2026",
            "AMOUNT PAID 456.50",
        ].join("\n");
        const p = parseReceipt(text);
        expect(p.merchant).toBe("Apollo Pharmacy");
        expect(p.amount).toBe(456.5);
        expect(ymd(p.date)).toBe("2026-07-01");
        expect(p.category).toBe("Medical"); // APOLLO/PHARMAC rule
    });

    it("expands two-digit years to 20xx", () => {
        const text = ["Some Shop", "DT 05/07/26", "TOTAL 99.00"].join("\n");
        const p = parseReceipt(text);
        expect(ymd(p.date)).toBe("2026-07-05");
    });

    it("prefers a date on a DATE-labelled line over other dates", () => {
        const text = [
            "Some Shop",
            "Valid till 31/12/2025",
            "Bill Date: 05/07/2026",
            "TOTAL 250.00",
        ].join("\n");
        const p = parseReceipt(text);
        expect(ymd(p.date)).toBe("2026-07-05");
    });

    it("uses OCR lines when provided and handles empty text", () => {
        const lines = [
            { text: "Zomato Order", top: 10 },
            { text: "TOTAL ₹ 420.00", top: 50 },
        ];
        const p = parseReceipt(lines.map((l) => l.text).join("\n"), lines);
        expect(p.merchant).toBe("Zomato Order");
        expect(p.amount).toBe(420);
        expect(p.category).toBe("Food"); // ZOMATO rule

        const empty = parseReceipt("");
        expect(empty.merchant).toBeNull();
        expect(empty.amount).toBeNull();
        expect(empty.date).toBeNull();
        expect(empty.category).toBe("Other");
    });
});
