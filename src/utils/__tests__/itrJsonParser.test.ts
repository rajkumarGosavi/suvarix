import { describe, expect, it } from "vitest";
import {
    ItrJsonUnsupported,
    normaliseAssessmentYear,
    parseItrJson,
    parseItrJsonObject,
    readBusinessIncome,
    regimeFromFilingStatus,
} from "@/utils/itrJsonParser";

// An ITD ITR-2 payload, cut down to the nodes the parser reads. Figures are
// invented but internally consistent, so the Part B invariants all balance.
function itr2Json(): Record<string, any> {
    return {
        ITR: {
            ITR2: {
                Form_ITR2: { FormName: "ITR2", AssessmentYear: "2026", SchemaVer: "Ver1.1" },
                PartA_GEN1: {
                    PersonalInfo: { PAN: "ABCDE1234F" },
                    FilingStatus: { OptOutNewTaxRegime: "Y" },
                },
                "PartB-TI": {
                    Salaries: 1250000,
                    IncomeFromHP: 180000,
                    CapGain: {
                        ShortTerm: { ShortTerm20Per: 45000, TotalShortTerm: 45000 },
                        LongTerm: { LongTerm12_5Per: 125000, TotalLongTerm: 125000 },
                        ShortTermLongTermTotal: 170000,
                        CapGains30Per115BBH: 0,
                        TotalCapGains: 170000,
                    },
                    IncFromOS: { OtherSrcThanOwnRaceHorse: 32500, TotIncFromOS: 32500 },
                    TotalTI: 1632500,
                    CurrentYearLoss: 0,
                    BalanceAfterSetoffLosses: 1632500,
                    BroughtFwdLossesSetoff: 0,
                    GrossTotalIncome: 1632500,
                    DeductionsUnderScheduleVIA: 150000,
                    TotalIncome: 1482500,
                },
                PartB_TTI: {
                    ComputationOfTaxLiability: {
                        TaxPayableOnTI: { TaxPayableOnTotInc: 247000 },
                        Rebate87A: 0,
                        TaxPayableOnRebate: 247000,
                        TotalSurcharge: 0,
                        EducationCess: 9880,
                        GrossTaxLiability: 256880,
                        NetTaxLiability: 256880,
                        IntrstPay: { IntrstPayUs234A: 0, TotalIntrstPay: 0 },
                        AggregateTaxInterestLiability: 256880,
                    },
                    TaxPaid: {
                        TaxesPaid: {
                            AdvanceTax: 20000,
                            TDS: 240000,
                            TCS: 0,
                            SelfAssessmentTax: 10000,
                            TotalTaxesPaid: 270000,
                        },
                        BalTaxPayable: 0,
                    },
                    Refund: { RefundDue: 13120 },
                },
                Verification: { Date: "2026-07-20", Place: "Pune" },
            },
        },
    };
}

/**
 * The same return filed on ITR-3: identical Part B, plus the profits-and-gains
 * head, with the income totals moved up by it.
 */
function itr3Json(): Record<string, any> {
    const json = itr2Json();
    const form = json.ITR.ITR2;
    delete json.ITR.ITR2;
    json.ITR.ITR3 = form;
    form.Form_ITR3 = { FormName: "ITR3", AssessmentYear: "2026", SchemaVer: "Ver1.1" };
    delete form.Form_ITR2;

    form["PartB-TI"].ProfBusGain = { ProfGainNoSpecBus: 400000, TotProfBusGain: 400000 };
    form["PartB-TI"].TotalTI = 2032500;
    form["PartB-TI"].BalanceAfterSetoffLosses = 2032500;
    form["PartB-TI"].GrossTotalIncome = 2032500;
    form["PartB-TI"].TotalIncome = 1882500;
    return json;
}

describe("parseItrJsonObject", () => {
    it("reads every Part B figure by schema path", () => {
        const { data } = parseItrJsonObject(itr2Json());

        expect(data.salaryIncome).toBe(1250000);
        expect(data.housePropertyIncome).toBe(180000);
        expect(data.capitalGainsStcg).toBe(45000);
        expect(data.capitalGainsLtcg).toBe(125000);
        expect(data.otherSourcesIncome).toBe(32500);
        expect(data.grossTotalIncome).toBe(1632500);
        expect(data.chapterViaDeductions).toBe(150000);
        expect(data.totalIncome).toBe(1482500);
        expect(data.taxOnTotalIncome).toBe(247000);
        expect(data.cess).toBe(9880);
        expect(data.totalTaxLiability).toBe(256880);
        expect(data.tdsPaid).toBe(240000);
        expect(data.advanceTaxPaid).toBe(20000);
        expect(data.selfAssessmentTaxPaid).toBe(10000);
        expect(data.totalTaxPaid).toBe(270000);
        expect(data.refundDue).toBe(13120);
        expect(data.taxPayable).toBe(0);
    });

    it("reads the header fields and masks the PAN", () => {
        const result = parseItrJsonObject(itr2Json());

        expect(result.formType).toBe("ITR-2");
        expect(result.assessmentYear).toBe("2026-27");
        expect(result.data.panMasked).toBe("XXXXX1234F");
        expect(result.data.filingDate).toBe("2026-07-20");
        expect(result.data.regime).toBe("old");
    });

    it("confirms every figure the invariants cover, with no issues", () => {
        const { confidence, issues } = parseItrJsonObject(itr2Json());

        expect(issues).toEqual([]);
        expect(confidence.totalIncome).toBe("confirmed");
        expect(confidence.grossTotalIncome).toBe("confirmed");
        expect(confidence.totalTaxPaid).toBe("confirmed");
        expect(confidence.refundDue).toBe("confirmed");
    });

    it("flags a figure that breaks the Part B arithmetic", () => {
        const json = itr2Json();
        json.ITR.ITR2["PartB-TI"].TotalIncome = 1400000;

        const { confidence, issues } = parseItrJsonObject(json);

        expect(issues.length).toBeGreaterThan(0);
        expect(confidence.totalIncome).toBe("conflict");
    });

    it("treats business income as zero — ITR-2 has no such head", () => {
        const { data, confidence } = parseItrJsonObject(itr2Json());

        expect(data.businessIncome).toBe(0);
        expect(confidence.businessIncome).not.toBe("missing");
    });

    it("marks an absent figure missing rather than guessing zero", () => {
        const json = itr2Json();
        delete json.ITR.ITR2["PartB-TI"].Salaries;
        delete json.ITR.ITR2["PartB-TI"].TotalTI;

        const { data, confidence } = parseItrJsonObject(json);

        expect(data.salaryIncome).toBeUndefined();
        expect(confidence.salaryIncome).toBe("missing");
    });

    it("derives a single missing figure from the surrounding equation", () => {
        const json = itr2Json();
        delete json.ITR.ITR2["PartB-TI"].TotalIncome;

        const { data, confidence } = parseItrJsonObject(json);

        expect(data.totalIncome).toBe(1482500);
        expect(confidence.totalIncome).toBe("derived");
    });

    it("flattens Part B into reviewable lines", () => {
        const { allLines } = parseItrJsonObject(itr2Json());

        expect(allLines).toContain("PartB-TI.CapGain.TotalCapGains = 170000");
        expect(allLines).toContain("PartB_TTI.TaxPaid.TaxesPaid.TDS = 240000");
    });

    it("rejects a form that is neither ITR-2 nor ITR-3", () => {
        expect(() => parseItrJsonObject({ ITR: { ITR1: {} } })).toThrow(ItrJsonUnsupported);
        expect(() => parseItrJsonObject({ ITR: { ITR5: {} } })).toThrow(ItrJsonUnsupported);
    });

    it("rejects JSON with no ITR node at all", () => {
        expect(() => parseItrJsonObject({ some: "other file" })).toThrow(ItrJsonUnsupported);
    });
});

describe("ITR-3", () => {
    it("reads the profits-and-gains head and balances the Part B totals", () => {
        const { data, confidence, issues, formType } = parseItrJsonObject(itr3Json());

        expect(formType).toBe("ITR-3");
        expect(data.businessIncome).toBe(400000);
        expect(data.totalIncome).toBe(1882500);
        expect(issues).toEqual([]);
        expect(confidence.grossTotalIncome).toBe("confirmed");
        expect(confidence.businessIncome).toBe("confirmed");
    });

    it("derives business income when the PGBP node is shaped unexpectedly", () => {
        const json = itr3Json();
        delete json.ITR.ITR3["PartB-TI"].ProfBusGain;

        const { data, confidence } = parseItrJsonObject(json);

        expect(data.businessIncome).toBe(400000);
        expect(confidence.businessIncome).toBe("derived");
    });

    it("keeps the other head-wise figures intact", () => {
        const { data } = parseItrJsonObject(itr3Json());

        expect(data.salaryIncome).toBe(1250000);
        expect(data.capitalGainsTotal).toBe(170000);
        expect(data.refundDue).toBe(13120);
    });
});

describe("readBusinessIncome", () => {
    it("prefers an explicit total over the components", () => {
        const form = {
            "PartB-TI": { ProfBusGain: { ProfGainNoSpecBus: 300000, TotProfBusGain: 400000 } },
        };
        expect(readBusinessIncome(form)?.value).toBe(400000);
    });

    it("sums the components when no total is present", () => {
        const form = {
            "PartB-TI": { ProfBusGain: { ProfGainNoSpecBus: 300000, ProfGainSpecBus: 100000 } },
        };
        expect(readBusinessIncome(form)?.value).toBe(400000);
    });

    it("returns undefined when there is no PGBP node", () => {
        expect(readBusinessIncome({ "PartB-TI": {} })).toBeUndefined();
    });
});

describe("parseItrJson", () => {
    it("parses the raw file text", () => {
        const result = parseItrJson(JSON.stringify(itr2Json()));
        expect(result.data.totalIncome).toBe(1482500);
    });

    it("rejects text that is not JSON", () => {
        expect(() => parseItrJson("%PDF-1.7")).toThrow(ItrJsonUnsupported);
    });
});

describe("normaliseAssessmentYear", () => {
    it("expands a single year to the ITD range", () => {
        expect(normaliseAssessmentYear("2026")).toBe("2026-27");
        expect(normaliseAssessmentYear("2099")).toBe("2099-00");
    });

    it("passes an already-formatted year through", () => {
        expect(normaliseAssessmentYear("2024-25")).toBe("2024-25");
    });

    it("returns null for anything else", () => {
        expect(normaliseAssessmentYear("AY 2026")).toBeNull();
    });
});

describe("regimeFromFilingStatus", () => {
    // The flag inverted when the new regime became the default: OptOutNewTaxRegime
    // (AY 2024-25 onwards) opts out, the older NewTaxRegime opted in.
    it("reads the opt-out flag", () => {
        expect(regimeFromFilingStatus({ OptOutNewTaxRegime: "Y" })).toBe("old");
        expect(regimeFromFilingStatus({ OptOutNewTaxRegime: "N" })).toBe("new");
    });

    it("reads the legacy opt-in flag", () => {
        expect(regimeFromFilingStatus({ NewTaxRegime: "Y" })).toBe("new");
        expect(regimeFromFilingStatus({ NewTaxRegime: "N" })).toBe("old");
    });

    it("returns undefined when neither flag is present", () => {
        expect(regimeFromFilingStatus({})).toBeUndefined();
        expect(regimeFromFilingStatus(undefined)).toBeUndefined();
    });
});
