import {
    applyInvariants,
    assumeNoBusinessHead,
    maskPan,
    type FieldConfidence,
    type ItrParseResult,
    type ParsedRow,
} from "./itrParser";

/**
 * Reads the ITD's own ITR JSON — the file the e-filing portal offers as
 * "Download JSON" and the offline utility writes out.
 *
 * Unlike the acknowledgement PDF this is structured data straight from the
 * department's schema (`docs/ITR-2_2026_Main_V1.1.json`), so there is no text to
 * scrape and no ambiguity about which row an amount belongs to. Every figure is
 * read by its schema path and lands at `parsed`; `applyInvariants` still runs so
 * the same equations that police the PDF also catch a hand-edited JSON.
 *
 * Two things the JSON does not carry and the PDF does:
 * - the acknowledgement number (assigned after upload, so never in the payload)
 * - a filing date — `Verification.Date` is used as the closest equivalent
 *
 * ITR-2 and ITR-3 share the whole Part B shape; the only difference is ITR-3's
 * profits-and-gains head, which `businessIncome` below reads separately.
 */

/** Thrown for a file that is JSON but not an ITR return we can read. */
export class ItrJsonUnsupported extends Error {
    constructor(message: string) {
        super(message);
        this.name = "ItrJsonUnsupported";
    }
}

type Json = Record<string, any>;

/** `get(obj, "PartB_TTI.TaxPaid.TaxesPaid.TDS")` — undefined at any missing link. */
function get(root: Json | undefined, path: string): unknown {
    let cur: any = root;
    for (const key of path.split(".")) {
        if (cur === null || typeof cur !== "object") return undefined;
        cur = cur[key];
    }
    return cur;
}

/**
 * Schema path → field. Paths are relative to the form node (`ITR.ITR2`).
 *
 * Audit fields are mapped too: they cost nothing here (the JSON prints every
 * roll-up) and they let the same invariants run as on a PDF import.
 */
const NUMBER_PATHS: Array<[keyof ParsedRow, string]> = [
    ["salaryIncome", "PartB-TI.Salaries"],
    ["housePropertyIncome", "PartB-TI.IncomeFromHP"],
    ["capitalGainsStcg", "PartB-TI.CapGain.ShortTerm.TotalShortTerm"],
    ["capitalGainsLtcg", "PartB-TI.CapGain.LongTerm.TotalLongTerm"],
    ["capitalGainsSum", "PartB-TI.CapGain.ShortTermLongTermTotal"],
    ["capitalGains115BBH", "PartB-TI.CapGain.CapGains30Per115BBH"],
    ["capitalGainsTotal", "PartB-TI.CapGain.TotalCapGains"],
    ["otherSourcesIncome", "PartB-TI.IncFromOS.TotIncFromOS"],
    ["totalHeadwiseIncome", "PartB-TI.TotalTI"],
    ["currentYearLossSetoff", "PartB-TI.CurrentYearLoss"],
    ["broughtFwdLossSetoff", "PartB-TI.BroughtFwdLossesSetoff"],
    ["grossTotalIncome", "PartB-TI.GrossTotalIncome"],
    ["chapterViaDeductions", "PartB-TI.DeductionsUnderScheduleVIA"],
    ["totalIncome", "PartB-TI.TotalIncome"],

    ["taxOnTotalIncome", "PartB_TTI.ComputationOfTaxLiability.TaxPayableOnTI.TaxPayableOnTotInc"],
    ["rebate87A", "PartB_TTI.ComputationOfTaxLiability.Rebate87A"],
    ["taxAfterRebate", "PartB_TTI.ComputationOfTaxLiability.TaxPayableOnRebate"],
    ["surcharge", "PartB_TTI.ComputationOfTaxLiability.TotalSurcharge"],
    ["cess", "PartB_TTI.ComputationOfTaxLiability.EducationCess"],
    ["totalTaxLiability", "PartB_TTI.ComputationOfTaxLiability.GrossTaxLiability"],
    ["netTaxLiability", "PartB_TTI.ComputationOfTaxLiability.NetTaxLiability"],
    ["interestAndFee", "PartB_TTI.ComputationOfTaxLiability.IntrstPay.TotalIntrstPay"],
    ["aggregateLiability", "PartB_TTI.ComputationOfTaxLiability.AggregateTaxInterestLiability"],

    ["tdsPaid", "PartB_TTI.TaxPaid.TaxesPaid.TDS"],
    ["advanceTaxPaid", "PartB_TTI.TaxPaid.TaxesPaid.AdvanceTax"],
    ["selfAssessmentTaxPaid", "PartB_TTI.TaxPaid.TaxesPaid.SelfAssessmentTax"],
    ["tcsPaid", "PartB_TTI.TaxPaid.TaxesPaid.TCS"],
    ["totalTaxPaid", "PartB_TTI.TaxPaid.TaxesPaid.TotalTaxesPaid"],

    ["taxPayable", "PartB_TTI.TaxPaid.BalTaxPayable"],
    ["refundDue", "PartB_TTI.Refund.RefundDue"],
];

/** Forms whose Part B this parser understands. */
const SUPPORTED_FORMS = new Set(["ITR-2", "ITR-3"]);

/**
 * The ITR-3 profits-and-gains head. Unlike every other figure this one is read
 * defensively: the ITD publishes a separate schema per form and we have only
 * verified ITR-2's locally, so the node is probed by name and then by shape.
 *
 * When neither works the field is simply left missing — the head-wise equation
 * has business income as a term, so a complete Part B recovers it as `derived`.
 */
const BUSINESS_TOTAL_PATHS = [
    "PartB-TI.ProfBusGain.TotProfBusGain",
    "PartB-TI.ProfBusGain.IncChargeableUnderPGBP",
    "PartB-TI.ProfBusGain.TotalProfBusGain",
    "PartB-TI.ProfitsAndGains",
    "PartB-TI.IncomeFromBusinessProf",
];

/**
 * Total business income, plus the path it came from for the audit trail.
 *
 * The `ProfBusGain` node holds the head's components (non-speculative,
 * speculative) and, in most versions, their total. A key starting `Tot` is taken
 * as that total; with no such key the components are summed, which is the same
 * number by construction.
 */
export function readBusinessIncome(form: Json): { value: number; path: string } | undefined {
    for (const path of BUSINESS_TOTAL_PATHS) {
        const v = get(form, path);
        if (typeof v === "number" && Number.isFinite(v)) return { value: v, path };
    }

    const node = get(form, "PartB-TI.ProfBusGain");
    if (!node || typeof node !== "object" || Array.isArray(node)) return undefined;

    const entries = Object.entries(node as Json).filter(
        ([, v]) => typeof v === "number" && Number.isFinite(v),
    ) as Array<[string, number]>;
    if (!entries.length) return undefined;

    const total = entries.find(([k]) => /^tot(al)?/i.test(k));
    if (total) return { value: total[1], path: `PartB-TI.ProfBusGain.${total[0]}` };

    return {
        value: entries.reduce((sum, [, v]) => sum + v, 0),
        path: `PartB-TI.ProfBusGain (sum of ${entries.map(([k]) => k).join(" + ")})`,
    };
}

/** "2026" → "2026-27". Already-hyphenated values pass through unchanged. */
export function normaliseAssessmentYear(raw: string): string | null {
    const trimmed = raw.trim();
    if (/^\d{4}-\d{2}$/.test(trimmed)) return trimmed;
    const m = trimmed.match(/^(\d{4})$/);
    if (!m) return null;
    const next = (Number(m[1]) + 1) % 100;
    return `${m[1]}-${String(next).padStart(2, "0")}`;
}

/**
 * Old / new regime from the filing-status flag.
 *
 * The flag flipped meaning when the new regime became the default: up to
 * AY 2023-24 `NewTaxRegime = Y` meant opting *in*, from AY 2024-25 the field is
 * `OptOutNewTaxRegime` and `Y` means opting *out*. Both are read so an older
 * return imports correctly.
 */
export function regimeFromFilingStatus(status: Json | undefined): "old" | "new" | undefined {
    const optOut = get(status, "OptOutNewTaxRegime");
    if (typeof optOut === "string" && /^[YN]$/i.test(optOut)) {
        return optOut.toUpperCase() === "Y" ? "old" : "new";
    }
    const legacy = get(status, "NewTaxRegime");
    if (typeof legacy === "string" && /^[YN]$/i.test(legacy)) {
        return legacy.toUpperCase() === "Y" ? "new" : "old";
    }
    return undefined;
}

/** `PartB-TI.CapGain.TotalCapGains = 68791` lines, for the review dialog + debug log. */
function flatten(node: unknown, prefix: string, out: string[]): void {
    if (node === null || node === undefined) return;
    if (typeof node !== "object") {
        out.push(`${prefix} = ${String(node)}`);
        return;
    }
    if (Array.isArray(node)) {
        node.forEach((v, i) => flatten(v, `${prefix}[${i}]`, out));
        return;
    }
    for (const [k, v] of Object.entries(node)) {
        flatten(v, prefix ? `${prefix}.${k}` : k, out);
    }
}

/**
 * Parses an ITD ITR JSON payload. Accepts the already-decoded object so the
 * caller owns the `JSON.parse` failure message.
 */
export function parseItrJsonObject(root: Json): ItrParseResult {
    const wrapper = (root?.ITR ?? root) as Json;
    if (!wrapper || typeof wrapper !== "object") {
        throw new ItrJsonUnsupported("This file is not an income-tax return JSON.");
    }

    // The root carries exactly one form node: ITR1 … ITR7.
    const formKey = Object.keys(wrapper).find((k) => /^ITR[1-7]$/i.test(k));
    if (!formKey) {
        throw new ItrJsonUnsupported(
            "No ITR form found in this JSON. Use the file downloaded from the e-filing " +
                "portal (Download JSON) or produced by the offline utility.",
        );
    }
    const form = wrapper[formKey] as Json;
    const formType = `ITR-${formKey.slice(3)}`;

    if (!SUPPORTED_FORMS.has(formType)) {
        throw new ItrJsonUnsupported(
            `This is an ${formType} JSON. Only ITR-2 and ITR-3 are supported — ` +
                "enter this return manually instead.",
        );
    }

    const data: ParsedRow = {};
    const confidence: Record<string, FieldConfidence> = {};
    const matchedLines: Record<string, string> = {};

    for (const [field, path] of NUMBER_PATHS) {
        const raw = get(form, path);
        if (typeof raw !== "number" || !Number.isFinite(raw)) {
            confidence[field] = "missing";
            continue;
        }
        (data as Record<string, unknown>)[field] = raw;
        confidence[field] = "parsed";
        matchedLines[field] = `${path} = ${raw}`;
    }

    // ITR-2 cannot report business income at all — the head does not exist on the
    // form, so zero is a fact about the return rather than a missing reading. On
    // ITR-3 it is read, and left missing (for the invariants to recover) if the
    // node is shaped differently from what we expect.
    if (assumeNoBusinessHead(formType)) {
        data.businessIncome = 0;
        confidence.businessIncome = "parsed";
        matchedLines.businessIncome = `(no business head on ${formType})`;
    } else {
        const business = readBusinessIncome(form);
        if (business) {
            data.businessIncome = business.value;
            confidence.businessIncome = "parsed";
            matchedLines.businessIncome = `${business.path} = ${business.value}`;
        } else {
            confidence.businessIncome = "missing";
        }
    }

    const pan = get(form, "PartA_GEN1.PersonalInfo.PAN");
    if (typeof pan === "string") {
        const masked = maskPan(pan);
        if (masked) {
            data.panMasked = masked;
            matchedLines.panMasked = "PartA_GEN1.PersonalInfo.PAN";
        }
    }

    // No filing date exists in the payload; the verification date is the day the
    // return was signed off, which is the same day for an e-filed return.
    const verifiedOn = get(form, "Verification.Date");
    if (typeof verifiedOn === "string" && /^\d{4}-\d{2}-\d{2}$/.test(verifiedOn)) {
        data.filingDate = verifiedOn;
        matchedLines.filingDate = `Verification.Date = ${verifiedOn}`;
    }

    const regime = regimeFromFilingStatus(get(form, "PartA_GEN1.FilingStatus") as Json | undefined);
    if (regime) {
        data.regime = regime;
        matchedLines.regime = "PartA_GEN1.FilingStatus";
    }

    for (const key of ["panMasked", "filingDate", "ackNumber", "regime"] as const) {
        confidence[key] = data[key] === undefined ? "missing" : "parsed";
    }

    const rawAy = get(form, `Form_${formKey}.AssessmentYear`);
    const assessmentYear =
        typeof rawAy === "string" || typeof rawAy === "number"
            ? normaliseAssessmentYear(String(rawAy))
            : null;

    const allLines: string[] = [];
    flatten(get(form, "PartB-TI"), "PartB-TI", allLines);
    flatten(get(form, "PartB_TTI"), "PartB_TTI", allLines);

    const checked = applyInvariants(data, confidence);

    return {
        data: checked.data,
        confidence: checked.confidence,
        issues: checked.issues,
        rawLines: allLines.slice(0, 300),
        allLines,
        matchedLines,
        formType,
        assessmentYear,
    };
}

/** Parses the raw text of a `.json` ITR file. */
export function parseItrJson(text: string): ItrParseResult {
    let root: Json;
    try {
        root = JSON.parse(text);
    } catch {
        throw new ItrJsonUnsupported("This file is not valid JSON.");
    }
    return parseItrJsonObject(root);
}
