import { describe, expect, it } from "vitest";
import {
    buildItrDebugReport,
    maskPan,
    parseItrLines,
    partBLines,
    applyInvariants,
} from "@/utils/itrParser";

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

// Schedule rows print before Part B and repeat the same labels. Amount rules must
// ignore them: the return-level figures only ever live in Part B-TI / Part B-TTI.
const SCHEDULE_NOISE = [
    "Schedule CG Capital Gains",
    "A Short-term capital gain 99,999",
    "B Long-term capital gain 88,888",
    "Schedule TDS1 Details of Tax Deducted at Source from Salary",
    "1 TDS 77,777",
];

describe("parseItrLines with schedule rows preceding Part B", () => {
    const withNoise = (() => {
        const at = ITR2_LINES.indexOf("Part B - TI");
        return [...ITR2_LINES.slice(0, at), ...SCHEDULE_NOISE, ...ITR2_LINES.slice(at)];
    })();

    it("reads capital gains from Part B, not from Schedule CG", () => {
        const { data } = parseItrLines(withNoise);
        expect(data.capitalGainsStcg).toBe(45000);
        expect(data.capitalGainsLtcg).toBe(125000);
    });

    it("reads TDS from Part B-TTI, not from Schedule TDS1", () => {
        expect(parseItrLines(withNoise).data.tdsPaid).toBe(240000);
    });

    it("still reads header fields, which live outside Part B", () => {
        const r = parseItrLines(withNoise);
        expect(r.formType).toBe("ITR-2");
        expect(r.data.panMasked).toBe("XXXXX1234F");
        expect(r.data.ackNumber).toBe("123456789012345");
    });
});

// Real ITR-V rows carry a cross-reference and repeat the item code between the
// label and the amount: "<code> <label> (<cross-ref>) <code> <amount>". Every
// number before the last one is structure, not money.
const ITR2_CODED_LINES = [
    "ITR-2 Assessment Year 2024-25",
    "PART B – TI COMPUTATION OF TOTAL INCOME",
    "1 Salaries (6 of Schedule S) 1 12,50,000",
    "2 Income from house property (3c of Schedule HP) 2 1,80,000",
    "3 a v Total short term (ai + aii + aiii + aiv) 3av 45,000",
    "3 b iv Total long term (bi + bii + biii) 3biv 1,25,000",
    "4 d Income from other sources (4a + 4b + 4c) 4d 32,500",
    "9 Gross Total Income (7 - 8) 9 16,32,500",
    "11 c Deductions under Chapter VI-A (11a + 11b) 11c 1,50,000",
    "13 Total Income (9 - 11c - 12) 13 14,82,500",
    "PART B-TTI COMPUTATION OF TAX LIABILITY ON TOTAL INCOME",
    "17 Refund (If 15e is greater than 14) 17 13,120",
];

describe("parseItrLines on rows with cross-references and repeated item codes", () => {
    it("takes the last number on the row as the amount", () => {
        const { data } = parseItrLines(ITR2_CODED_LINES);
        expect(data.salaryIncome).toBe(1250000);
        expect(data.housePropertyIncome).toBe(180000);
        expect(data.otherSourcesIncome).toBe(32500);
    });

    it("reads the capital gains totals, not the sub-rate rows", () => {
        const { data } = parseItrLines(ITR2_CODED_LINES);
        expect(data.capitalGainsStcg).toBe(45000);
        expect(data.capitalGainsLtcg).toBe(125000);
    });

    it("does not mistake a cross-reference for the amount", () => {
        const { data } = parseItrLines(ITR2_CODED_LINES);
        expect(data.grossTotalIncome).toBe(1632500);
        expect(data.totalIncome).toBe(1482500);
        expect(data.chapterViaDeductions).toBe(150000);
        expect(data.refundDue).toBe(13120);
    });

    it("skips a label row that carries no amount", () => {
        const r = parseItrLines([
            "PART B – TI COMPUTATION OF TOTAL INCOME",
            "3 Capital Gains",
            "3 a v Total short term (ai + aii + aiii + aiv) 3av 45,000",
        ]);
        expect(r.data.capitalGainsStcg).toBe(45000);
    });
});

// Row shapes taken from a real AY 2026-27 ITR-2 acknowledgement (figures invented).
// Two things make this layout hostile: heading rows carry an item code but no money
// column, and long rows wrap so the code and amount land on a line of their own.
const ITR2_WRAPPED_LINES = [
    "ITR-2 Assessment Year 2026-27",
    "PART B – TI COMPUTATION OF TOTAL INCOME",
    "1 Salaries (6 of Schedule S) 1 42,17,425",
    "2 Income from house property (3 of Schedule-HP) (Enter nil if loss) 2 0",
    "3 Capital Gains 3",
    "a Short-term 3a",
    "i Short term chargeable @20% (8ii of item E of Sch CG) ai 0",
    "v Total Short term (ai + aii + aiii + aiv) (enter nil if loss) av 0",
    "b Long-term 3b",
    "i Long-term chargeable @ 12.5% (8vi of item E of schedule CG) bi 68,791",
    "Total Long term (bi + bii) (enter nil if loss)",
    "iii biii 68,791",
    "4 Income from other sources 4",
    "a Net Income from Other sources chargeable to tax at Normal Applicable rates (6 of 4a 90,000",
    "d Total (4a + 4b + 4c)(enter nil if loss) 4d 1,08,227",
    "c Sum of Short-term / Long-term Capital Gains (3av + 3biii) (enter nil if loss) 3c 68,791",
    "d Capital gains chargeable @ 30 % u/s 115BBH (C2 of Schedule CG) 3d 0",
    "e Total Capital Gains (3c+3d) 3e 68,791",
    "5 Total of head wise income (1 + 2 + 3e + 4d) 5 43,94,443",
    "6 Losses of current year set off against 5 (total of 2xiii and 3xiii of Schedule CYLA) 6 0",
    "8 Brought forward losses set off against 7 ( 2xii of Schedule BFLA) 8 0",
    "9 Gross Total income (7-8) (3xiii of Schedule BFLA + 2 of Schedule OS ) 9 43,94,443",
    "11 Deductions under Chapter VI-A [v of Schedule VIA and limited to (9-10)] 11 0",
    "12 Total income (9 - 11) 12 43,94,440",
    "PARTB-TTI - COMPUTATION OF TAX LIABILITY ON TOTAL INCOME",
    "2 Tax payable on total income 2",
    "d Tax Payable on Total Income (2a + 2b -2c) 2d 8,77,696",
    "c Health and Education Cess @ 4% on (1a+1b) above 1c 0",
    "3 Rebate under section 87A 3 0",
    "4 Tax Payable after rebate (2d-3) 4 8,77,696",
    "5 Surcharge 5",
    "ii 10% or 15% as applicable 5ii 0",
    "iii Total (ia + iia) 5iii 0",
    "6 Health and Education cess @ 4% on (4 + 5iv) 6 35,108",
    "7 Gross tax liability (4 + 5iv + 6) 7 9,12,804",
    "12 Net tax liability (10-11d) (enter zero if negative) 12 9,12,804",
    "a Interest for default in furnishing the return (section 234A) 13a 0",
    "Interest for default in payment of advance tax (section 234B) 13b",
    "b 0",
    "a Advance Tax (from column 5 of 20A) 15a 0",
    "b TDS (total of column 5 of 20B and column 9 of 20C) 15b 9,26,670",
    "c TCS (total of column 7(i) of 20D) 15cc 0",
    "d Self Assessment Tax (from column 5 of 20A) 15d 0",
    "e Total Interest and Fee Payable (13a+13b+13c+13d+13da) 13e 0",
    "14 Aggregate liability (12+13e) 14 9,12,804",
    "e Total Taxes Paid (15a+15b+15c+15d) 15e 9,26,670",
    "16 Amount payable (Enter if 14 is greater than 15e, else enter 0) 16 0",
    "17 Refund (If 15e is greater than 14) 17 13,870",
];

describe("parseItrLines on the wrapped real-world layout", () => {
    it("ignores a heading row whose trailing token is an item code, not money", () => {
        const { data } = parseItrLines(ITR2_WRAPPED_LINES);
        expect(data.capitalGainsStcg).toBe(0);
        expect(data.taxOnTotalIncome).toBe(877696);
    });

    it("reads a total whose amount wrapped onto the next line", () => {
        expect(parseItrLines(ITR2_WRAPPED_LINES).data.capitalGainsLtcg).toBe(68791);
    });

    it("prefers the other-sources total over its first sub-row", () => {
        expect(parseItrLines(ITR2_WRAPPED_LINES).data.otherSourcesIncome).toBe(108227);
    });

    it("reads the cess on tax, not the cess on deemed income under 115JC", () => {
        expect(parseItrLines(ITR2_WRAPPED_LINES).data.cess).toBe(35108);
    });

    it("reads Advance Tax paid, not the 234B interest row that names it", () => {
        const r = parseItrLines(ITR2_WRAPPED_LINES);
        expect(r.data.advanceTaxPaid).toBe(0);
        expect(r.matchedLines.advanceTaxPaid).toMatch(/15a/);
    });

    it("tolerates the section 288A rounding of total income to the nearest ten", () => {
        const r = parseItrLines(ITR2_WRAPPED_LINES);
        expect(r.data.totalIncome).toBe(4394440);
        expect(r.confidence.totalIncome).toBe("confirmed");
        expect(r.issues).toEqual([]);
    });

    it("reads the whole taxes-paid block", () => {
        const { data } = parseItrLines(ITR2_WRAPPED_LINES);
        expect(data.tdsPaid).toBe(926670);
        expect(data.tcsPaid).toBe(0);
        expect(data.selfAssessmentTaxPaid).toBe(0);
        expect(data.totalTaxPaid).toBe(926670);
        expect(data.refundDue).toBe(13870);
        expect(data.taxPayable).toBe(0);
    });
});

describe("audit rows and the equations they close", () => {
    it("reads the surcharge roll-up, not the 115JC surcharge row", () => {
        expect(parseItrLines(ITR2_WRAPPED_LINES).data.surcharge).toBe(0);
        expect(parseItrLines(ITR2_WRAPPED_LINES).matchedLines.surcharge).toMatch(/5iii/);
    });

    it("confirms both capital gains figures against the 3c roll-up", () => {
        const r = parseItrLines(ITR2_WRAPPED_LINES);
        expect(r.confidence.capitalGainsStcg).toBe("confirmed");
        expect(r.confidence.capitalGainsLtcg).toBe("confirmed");
    });

    it("confirms tax on total income against the after-rebate row", () => {
        expect(parseItrLines(ITR2_WRAPPED_LINES).confidence.taxOnTotalIncome).toBe("confirmed");
    });

    it("confirms the gross tax liability build-up", () => {
        const r = parseItrLines(ITR2_WRAPPED_LINES);
        expect(r.confidence.totalTaxLiability).toBe("confirmed");
        expect(r.confidence.cess).toBe("confirmed");
        expect(r.confidence.surcharge).toBe("confirmed");
    });

    it("confirms the head-wise income total feeds gross total income", () => {
        const r = parseItrLines(ITR2_WRAPPED_LINES);
        expect(r.confidence.salaryIncome).toBe("confirmed");
        expect(r.confidence.otherSourcesIncome).toBe("confirmed");
        expect(r.confidence.grossTotalIncome).toBe("confirmed");
    });

    it("reconciles the refund against liability minus taxes paid", () => {
        const r = parseItrLines(ITR2_WRAPPED_LINES);
        expect(r.confidence.refundDue).toBe("confirmed");
        expect(r.issues).toEqual([]);
    });

    it("keeps audit rows out of the saved payload's own fields", () => {
        const { data } = parseItrLines(ITR2_WRAPPED_LINES);
        expect(data.capitalGainsTotal).toBe(68791);
        expect(data.aggregateLiability).toBe(912804);
        expect(data.rebate87A).toBe(0);
    });

    it("flags a capital gains figure that contradicts the 3c roll-up", () => {
        const broken = ITR2_WRAPPED_LINES.map((l) =>
            /^iii biii/.test(l) ? "iii biii 99,999" : l,
        );
        const r = parseItrLines(broken);
        expect(r.confidence.capitalGainsLtcg).toBe("conflict");
        expect(r.issues.some((i) => i.fields.includes("capitalGainsLtcg"))).toBe(true);
    });
});

describe("partBLines", () => {
    it("slices from the Part B-TI header to the TAX PAYMENTS section", () => {
        const lines = [
            "Header noise",
            "PART B – TI COMPUTATION OF TOTAL INCOME",
            "1 Salaries (6 of Schedule S) 1 12,50,000",
            "PARTB-TTI - COMPUTATION OF TAX LIABILITY ON TOTAL INCOME",
            "17 Refund (If 15e is greater than 14) 17 13,120",
            "TAX PAYMENTS",
            "20A Advance tax table",
        ];
        const slice = partBLines(lines);
        expect(slice[0]).toContain("PART B – TI");
        expect(slice).toHaveLength(4);
        expect(slice.some((l) => /TAX PAYMENTS/.test(l))).toBe(false);
    });

    it("falls back to every line when the section header is absent", () => {
        expect(partBLines(["a", "b"])).toEqual(["a", "b"]);
    });
});

describe("buildItrDebugReport", () => {
    it("lists Part B raw lines and every field with its source line", () => {
        const result = parseItrLines(ITR2_LINES);
        const report = buildItrDebugReport(result, "itr2.pdf");

        expect(report).toContain("file: itr2.pdf");
        expect(report).toContain("formType: ITR-2");
        expect(report).toContain("assessmentYear: 2024-25");
        expect(report).toContain("--- RAW LINES: PART B-TI + PART B-TTI ---");
        expect(report).toContain("1 Salaries (6 of Schedule S) 12,50,000");
        expect(report).toMatch(/salaryIncome\s+= 1250000/);
        expect(report).toContain('← "1 Salaries (6 of Schedule S) 12,50,000"');
    });

    it("records each field's confidence alongside its value", () => {
        const report = buildItrDebugReport(parseItrLines(ITR2_LINES), "itr2.pdf");
        expect(report).toMatch(/totalIncome\s+= 1482500 \[confirmed]/);
    });

    it("lists the equations that did not balance", () => {
        const broken = ITR2_LINES.map((l) =>
            /^Total Income/.test(l) ? "Total Income 9,99,999" : l,
        );
        const report = buildItrDebugReport(parseItrLines(broken), "itr2.pdf");
        expect(report).toContain("--- FAILED INVARIANTS ---");
        expect(report).toContain("expected 1482500, read 999999");
    });

    it("marks fields the parser could not find", () => {
        const partial = ITR2_LINES.filter((l) => !/house property/i.test(l));
        const report = buildItrDebugReport(parseItrLines(partial), "itr2.pdf");
        expect(report).toMatch(/housePropertyIncome\s+= MISSING/);
    });
});

describe("applyInvariants", () => {
    it("confirms every term of an equation that balances", () => {
        const r = applyInvariants(
            { grossTotalIncome: 1632500, chapterViaDeductions: 150000, totalIncome: 1482500 },
            { grossTotalIncome: "parsed", chapterViaDeductions: "parsed", totalIncome: "parsed" },
        );
        expect(r.confidence.totalIncome).toBe("confirmed");
        expect(r.confidence.grossTotalIncome).toBe("confirmed");
        expect(r.confidence.chapterViaDeductions).toBe("confirmed");
        expect(r.issues).toEqual([]);
    });

    it("derives the one missing term of an otherwise complete equation", () => {
        const r = applyInvariants(
            { grossTotalIncome: 1632500, chapterViaDeductions: 150000 },
            { grossTotalIncome: "parsed", chapterViaDeductions: "parsed", totalIncome: "missing" },
        );
        expect(r.data.totalIncome).toBe(1482500);
        expect(r.confidence.totalIncome).toBe("derived");
    });

    it("derives a missing addend, not just the total", () => {
        const r = applyInvariants(
            { totalTaxPaid: 270000, tdsPaid: 240000, advanceTaxPaid: 20000, tcsPaid: 0 },
            {
                totalTaxPaid: "parsed", tdsPaid: "parsed", advanceTaxPaid: "parsed",
                tcsPaid: "parsed", selfAssessmentTaxPaid: "missing",
            },
        );
        expect(r.data.selfAssessmentTaxPaid).toBe(10000);
        expect(r.confidence.selfAssessmentTaxPaid).toBe("derived");
    });

    it("flags every term when a complete equation does not balance", () => {
        const r = applyInvariants(
            { grossTotalIncome: 1632500, chapterViaDeductions: 150000, totalIncome: 999999 },
            { grossTotalIncome: "parsed", chapterViaDeductions: "parsed", totalIncome: "parsed" },
        );
        expect(r.confidence.totalIncome).toBe("conflict");
        expect(r.issues).toHaveLength(1);
        expect(r.issues[0].expected).toBe(1482500);
        expect(r.issues[0].actual).toBe(999999);
        expect(r.issues[0].fields).toContain("grossTotalIncome");
    });

    it("leaves an equation alone when two terms are missing", () => {
        const r = applyInvariants(
            { grossTotalIncome: 1632500 },
            { grossTotalIncome: "parsed", chapterViaDeductions: "missing", totalIncome: "missing" },
        );
        expect(r.data.totalIncome).toBeUndefined();
        expect(r.confidence.totalIncome).toBe("missing");
        expect(r.issues).toEqual([]);
    });

    it("rejects a negative amount — every ITR money field is a non-negative integer", () => {
        const r = applyInvariants({ salaryIncome: -500 }, { salaryIncome: "parsed" });
        expect(r.confidence.salaryIncome).toBe("conflict");
    });

    it("does not downgrade a field already confirmed by another equation", () => {
        const r = applyInvariants(
            {
                grossTotalIncome: 1632500, chapterViaDeductions: 150000, totalIncome: 1482500,
                totalTaxPaid: 270000, tdsPaid: 240000, advanceTaxPaid: 20000,
                selfAssessmentTaxPaid: 10000, tcsPaid: 0,
            },
            {},
        );
        expect(r.confidence.totalIncome).toBe("confirmed");
        expect(r.confidence.totalTaxPaid).toBe("confirmed");
        expect(r.issues).toEqual([]);
    });
});

describe("parseItrLines invariant integration", () => {
    it("confirms parsed fields that satisfy the Part B arithmetic", () => {
        const r = parseItrLines(ITR2_LINES);
        expect(r.confidence.totalIncome).toBe("confirmed");
        expect(r.confidence.totalTaxPaid).toBe("confirmed");
        expect(r.issues).toEqual([]);
    });

    it("recovers a field the PDF text lost", () => {
        const r = parseItrLines(ITR2_LINES.filter((l) => !/^Total Income/.test(l)));
        expect(r.data.totalIncome).toBe(1482500);
        expect(r.confidence.totalIncome).toBe("derived");
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
