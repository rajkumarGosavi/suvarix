import { describe, expect, it } from "vitest";
import { maskPan, parseItrLines } from "@/utils/itrParser";

// Representative flattened text lines from an ITR-2 PDF (label, then value).
const ITR2_LINES = [
    "INDIAN INCOME TAX RETURN ACKNOWLEDGEMENT",
    "ITR-2 Assessment Year 2024-25",
    "PAN ABCDE1234F",
    "Acknowledgement Number 123456789012345 Date of filing 20-Jul-2024",
    "Are you opting for new tax regime u/s 115BAC ? No",
    "Part B - TI",
    "1 Salaries (6 of Schedule S) 12,50,000",
    "2 Income from house property (3c of Schedule HP) 1,80,000",
    "3 Capital Gains",
    "a Short-term 45,000",
    "b Long-term 1,25,000",
    "4 Income from other sources 32,500",
    "5 Total (1+2+3+4) 16,32,500",
    "Gross Total Income 16,32,500",
    "Deductions under Chapter VI-A 1,50,000",
    "Total Income 14,82,500",
    "Part B - TTI",
    "1 Tax payable on total income 2,47,000",
    "2 Surcharge 0",
    "3 Health and Education Cess 9,880",
    "4 Gross tax liability 2,56,880",
    "Total Taxes Paid 2,70,000",
    "a TDS 2,40,000",
    "b Advance Tax 20,000",
    "c Self Assessment Tax 10,000",
    "d TCS 0",
    "Refund 13,120",
];

describe("parseItrLines", () => {
    it("reads the header: form type, assessment year, masked PAN, filing date, regime", () => {
        const r = parseItrLines(ITR2_LINES);
        expect(r.formType).toBe("ITR-2");
        expect(r.assessmentYear).toBe("2024-25");
        expect(r.data.panMasked).toBe("XXXXX1234F");
        expect(r.data.ackNumber).toBe("123456789012345");
        expect(r.data.filingDate).toBe("2024-07-20");
        expect(r.data.regime).toBe("old");
    });

    it("reads the income heads", () => {
        const { data } = parseItrLines(ITR2_LINES);
        expect(data.salaryIncome).toBe(1250000);
        expect(data.housePropertyIncome).toBe(180000);
        expect(data.capitalGainsStcg).toBe(45000);
        expect(data.capitalGainsLtcg).toBe(125000);
        expect(data.otherSourcesIncome).toBe(32500);
        expect(data.grossTotalIncome).toBe(1632500);
    });

    it("reads deductions and the tax block", () => {
        const { data } = parseItrLines(ITR2_LINES);
        expect(data.chapterViaDeductions).toBe(150000);
        expect(data.totalIncome).toBe(1482500);
        expect(data.taxOnTotalIncome).toBe(247000);
        expect(data.surcharge).toBe(0);
        expect(data.cess).toBe(9880);
        expect(data.totalTaxLiability).toBe(256880);
    });

    it("reads taxes paid and refund", () => {
        const { data } = parseItrLines(ITR2_LINES);
        expect(data.tdsPaid).toBe(240000);
        expect(data.advanceTaxPaid).toBe(20000);
        expect(data.selfAssessmentTaxPaid).toBe(10000);
        expect(data.tcsPaid).toBe(0);
        expect(data.totalTaxPaid).toBe(270000);
        expect(data.refundDue).toBe(13120);
    });

    it("marks unmatched fields missing instead of defaulting them to zero", () => {
        const partial = ITR2_LINES.filter((l) => !/house property/i.test(l));
        const r = parseItrLines(partial);
        expect(r.data.housePropertyIncome).toBeUndefined();
        expect(r.confidence.housePropertyIncome).toBe("missing");
        expect(r.confidence.salaryIncome).toBe("parsed");
    });

    it("detects the new regime", () => {
        const lines = ITR2_LINES.map((l) =>
            /115BAC/.test(l) ? "Are you opting for new tax regime u/s 115BAC ? Yes" : l,
        );
        expect(parseItrLines(lines).data.regime).toBe("new");
    });

    it("returns null form type and year for a non-ITR document", () => {
        const r = parseItrLines(["Some bank statement", "Closing balance 1,000.00"]);
        expect(r.formType).toBeNull();
        expect(r.assessmentYear).toBeNull();
    });
});

describe("maskPan", () => {
    it("keeps only the last five characters", () => {
        expect(maskPan("ABCDE1234F")).toBe("XXXXX1234F");
    });
    it("returns an empty string for junk input", () => {
        expect(maskPan("nope")).toBe("");
    });
});
