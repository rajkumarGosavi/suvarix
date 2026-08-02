import * as pdfjsLib from "pdfjs-dist";
import type { TextItem } from "pdfjs-dist/types/src/display/api";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";

pdfjsLib.GlobalWorkerOptions.workerSrc = workerUrl;

/** Numeric + text fields the parser can fill. Mirrors the Rust `ItrReturn` payload. */
export interface ParsedItr {
    regime?: "old" | "new";
    panMasked?: string;
    filingDate?: string;
    ackNumber?: string;

    salaryIncome?: number;
    housePropertyIncome?: number;
    capitalGainsStcg?: number;
    capitalGainsLtcg?: number;
    otherSourcesIncome?: number;
    businessIncome?: number;
    grossTotalIncome?: number;

    chapterViaDeductions?: number;
    totalIncome?: number;

    taxOnTotalIncome?: number;
    surcharge?: number;
    cess?: number;
    totalTaxLiability?: number;

    tdsPaid?: number;
    advanceTaxPaid?: number;
    selfAssessmentTaxPaid?: number;
    tcsPaid?: number;
    totalTaxPaid?: number;

    refundDue?: number;
    taxPayable?: number;
}

/**
 * Part B rows read purely to cross-check the figures above. They are never saved:
 * `ItrImportDialog` copies only the fields it lists, so these stay in the parse
 * result and go no further. Without them most equations have too many unknowns to
 * solve, which is the only reason they are parsed at all.
 */
export interface ItrAuditFields {
    /** 3c — sum of short- and long-term gains, before the 115BBH bucket. */
    capitalGainsSum?: number;
    /** 3d — gains taxed at 30% u/s 115BBH (virtual digital assets). */
    capitalGains115BBH?: number;
    /** 3e — total capital gains, as it enters the head-wise total. */
    capitalGainsTotal?: number;
    /** 5 — total of head-wise income, before any loss set-off. */
    totalHeadwiseIncome?: number;
    /** 6 — current-year losses set off (Schedule CYLA). */
    currentYearLossSetoff?: number;
    /** 8 — brought-forward losses set off (Schedule BFLA). */
    broughtFwdLossSetoff?: number;
    /** 3 of Part B-TTI — rebate u/s 87A. */
    rebate87A?: number;
    /** 4 of Part B-TTI — tax payable after rebate. */
    taxAfterRebate?: number;
    /** 12 of Part B-TTI — net tax liability, before interest and fee. */
    netTaxLiability?: number;
    /** 13e — total interest u/s 234A/B/C plus fee u/s 234F. */
    interestAndFee?: number;
    /** 14 — aggregate liability, which the taxes paid are settled against. */
    aggregateLiability?: number;
}

/** Everything a Part B row can populate: the saved payload plus the audit rows. */
export type ParsedRow = ParsedItr & ItrAuditFields;

/**
 * How much a field is trusted, weakest first:
 *
 * - `missing`   — no row matched, and no equation could recover it
 * - `parsed`    — a row matched, but no equation covers the field
 * - `derived`   — not in the text; computed as the sole unknown of an equation
 * - `confirmed` — parsed *and* an equation it appears in balances exactly
 * - `conflict`  — an equation it appears in does not balance, or the value is
 *                 negative (every Part B money field is a non-negative integer)
 */
export type FieldConfidence = "missing" | "parsed" | "derived" | "confirmed" | "conflict";

const CONFIDENCE_RANK: Record<FieldConfidence, number> = {
    missing: 0, parsed: 1, derived: 2, confirmed: 3, conflict: 4,
};

/** A Part B equation that did not balance, with the terms that fed it. */
export interface InvariantIssue {
    equation: string;
    expected: number;
    actual: number;
    fields: string[];
}

export interface ItrParseResult {
    data: ParsedRow;
    confidence: Record<string, FieldConfidence>;
    /** Equations that failed — every field they touch is marked `conflict`. */
    issues: InvariantIssue[];
    /** First 300 flattened lines, shown in the review dialog for debugging. */
    rawLines: string[];
    /** Every flattened line, kept for the debug report (not shown in the UI). */
    allLines: string[];
    /** For each parsed field, the source line it was read from. */
    matchedLines: Record<string, string>;
    formType: string | null;
    assessmentYear: string | null;
}

/** Thrown when the PDF is encrypted so the UI can prompt for the password. */
export class ItrPasswordRequired extends Error {
    constructor() {
        super("This ITR PDF is password protected.");
        this.name = "ItrPasswordRequired";
    }
}

// ─── helpers ─────────────────────────────────────────────────

const AMOUNT = String.raw`\(?-?[\d,]+(?:\.\d{1,2})?\)?`;
/** A whole token that is nothing but a number — "68,791" yes, "3b" and "15cc" no. */
const WHOLE_AMOUNT = new RegExp(`^${AMOUNT}$`);
/** `… <item code> <amount>` at the end of a row: "… 15b 9,26,670", "… av 0". */
const ROW_TAIL = new RegExp(String.raw`(?:^|\s)(?:\d{1,2}[a-z]{0,4}|[a-z]{1,4})\s+(${AMOUNT})\s*$`, "i");
/**
 * A section heading: the leading enumerator repeats as the final token, with no
 * money column between them — "3 Capital Gains 3", "15 Taxes Paid 15". The repeat
 * is what separates these from a real row like "2 Surcharge 0".
 */
const HEADING_ROW = /^(\S+)\s+.*\s(\S+)$/;

function parseAmount(raw: string): number | undefined {
    const negative = raw.trim().startsWith("(");
    const n = parseFloat(raw.replace(/[(),]/g, ""));
    if (!Number.isFinite(n)) return undefined;
    return negative ? -n : n;
}

/**
 * The money column of a Part B row, or `undefined` when the row carries none.
 *
 * Real acknowledgements print `<enumerator> <label> (<cross-ref>) <item code> <amount>`,
 * so most numbers on a line are structure. Worse, heading rows ("b Long-term 3b")
 * end in an item code that looks like money if you simply take the last number —
 * which is how long-term capital gains used to parse as `3`.
 */
function rowAmount(line: string): number | undefined {
    const heading = line.match(HEADING_ROW);
    if (heading && heading[1] === heading[2]) return undefined;

    const tail = line.match(ROW_TAIL);
    if (tail) return parseAmount(tail[1]);

    // Layouts that print no item-code column still end in the amount itself.
    const last = line.trim().split(/\s+/).pop() ?? "";
    return WHOLE_AMOUNT.test(last) ? parseAmount(last) : undefined;
}

/** Enumerators and an amount, with no label text — "iii biii 68,791", "b 0". */
const BARE_CONTINUATION = new RegExp(String.raw`^(?:[0-9a-z]{1,5}\s+){0,3}${AMOUNT}\s*$`, "i");

/**
 * Rejoins rows the PDF wrapped. A long label pushes its item code and amount onto
 * the following line, leaving "Total Long term (bi + bii) (enter nil if loss)" with
 * no figure and "iii biii 68,791" with no label.
 *
 * Only a line with no label text at all is treated as a continuation, so a genuine
 * next row ("i Short term chargeable @20% … ai 0") is never swallowed.
 */
export function joinWrappedRows(lines: string[]): string[] {
    const out: string[] = [];
    for (const line of lines) {
        const prev = out[out.length - 1];
        if (
            prev !== undefined &&
            BARE_CONTINUATION.test(line) &&
            rowAmount(prev) === undefined &&
            rowAmount(line) !== undefined
        ) {
            out[out.length - 1] = `${prev} ${line}`;
            continue;
        }
        out.push(line);
    }
    return out;
}

/** "XXXXX1234F" — only the last five PAN characters are ever stored. */
export function maskPan(pan: string): string {
    const m = pan.trim().toUpperCase().match(/[A-Z]{5}\d{4}[A-Z]/);
    if (!m) return "";
    return `XXXXX${m[0].slice(5)}`;
}

const MONTHS: Record<string, string> = {
    jan: "01", feb: "02", mar: "03", apr: "04", may: "05", jun: "06",
    jul: "07", aug: "08", sep: "09", oct: "10", nov: "11", dec: "12",
};

/** "20-Jul-2024" or "20/07/2024" → "2024-07-20"; anything else → undefined. */
function parseDate(raw: string): string | undefined {
    const named = raw.match(/(\d{1,2})[-/\s]([A-Za-z]{3})[a-z]*[-/\s](\d{4})/);
    if (named) {
        const mm = MONTHS[named[2].toLowerCase()];
        if (mm) return `${named[3]}-${mm}-${named[1].padStart(2, "0")}`;
    }
    const numeric = raw.match(/(\d{1,2})[-/](\d{1,2})[-/](\d{4})/);
    if (numeric) {
        return `${numeric[3]}-${numeric[2].padStart(2, "0")}-${numeric[1].padStart(2, "0")}`;
    }
    return undefined;
}

/**
 * Label rules. Patterns match the row's *label* only — the amount always comes
 * from `lastAmount`, never from a capture group, because the number sitting next
 * to a label is usually a cross-reference.
 *
 * The first pattern that matches a line carrying an amount wins, so order the
 * roll-up row ("Total short term") ahead of the per-rate rows it sums.
 */
const AMOUNT_RULES: Array<{ field: keyof ParsedRow; patterns: RegExp[] }> = [
    { field: "salaryIncome", patterns: [
        /Income\s+from\s+Salary\s*\/?\s*Pension/i,
        /Salaries?\b/i,
    ]},
    { field: "housePropertyIncome", patterns: [
        /Income\s+from\s+house\s+property/i,
    ]},
    { field: "capitalGainsStcg", patterns: [
        /Total\s+short[-\s]?term/i,
        /Short[-\s]?term\s*(?:capital\s+gains?)?/i,
    ]},
    { field: "capitalGainsLtcg", patterns: [
        /Total\s+long[-\s]?term/i,
        /Long[-\s]?term\s*(?:capital\s+gains?)?/i,
    ]},
    // The 4d roll-up carries no head name, only "Total (4a + 4b + 4c)", so it has to
    // be matched by its cross-reference or the 4a sub-row wins.
    { field: "otherSourcesIncome", patterns: [
        /Total\s*\(\s*4a\s*\+/i,
        /Income\s+from\s+other\s+sources/i,
    ]},
    { field: "businessIncome", patterns: [
        /Profits?\s+and\s+gains?\s+from\s+business\s+or\s+profession/i,
    ]},
    { field: "grossTotalIncome", patterns: [
        /Gross\s+Total\s+Income/i,
    ]},
    { field: "chapterViaDeductions", patterns: [
        /Deductions?\s+under\s+Chapter\s+VI[-\s]?A/i,
    ]},
    // "Total Income" must start the label: the lookbehind rejects both
    // "Gross Total Income" and "Tax payable on total income".
    { field: "totalIncome", patterns: [
        /(?<![A-Za-z]\s)Total\s+Income/i,
    ]},
    { field: "taxOnTotalIncome", patterns: [
        /Tax\s+payable\s+on\s+total\s+income/i,
    ]},
    // Like 4d, the surcharge roll-up (5iii) is labelled only "Total (ia + iia)", so
    // without the cross-reference this lands on the 115JC surcharge row (1b) instead.
    { field: "surcharge", patterns: [
        /Total\s*\(\s*ia\s*\+/i,
        /Total\s+Surcharge/i,
        /Surcharge/i,
    ]},
    // Part B-TTI prints cess twice: once on the deemed income under 115JC ("on
    // (1a+1b)") and once on the real liability ("on (4 + 5iv)"). The 115JC row comes
    // first and is almost always zero, so the cess on tax has to be preferred.
    { field: "cess", patterns: [
        /Cess\s*@?\s*\d*\s*%?\s*on\s*\(\s*4\b/i,
        /(?:Health\s+and\s+Education\s+)?Cess/i,
    ]},
    { field: "totalTaxLiability", patterns: [
        /Gross\s+tax\s+liability/i,
        /Total\s+Tax\s+(?:and\s+Interest\s+)?Liability/i,
    ]},
    { field: "tdsPaid", patterns: [
        /\bTDS\b/i,
    ]},
    // "…default in payment of advance tax (section 234B)" is an interest row, not a
    // payment; the lookbehind keeps the 234B/234C rows out.
    { field: "advanceTaxPaid", patterns: [
        /(?<!of\s)Advance\s+Tax/i,
    ]},
    { field: "selfAssessmentTaxPaid", patterns: [
        /Self[-\s]?Assessment\s+Tax/i,
    ]},
    { field: "tcsPaid", patterns: [
        /\bTCS\b/i,
    ]},
    { field: "totalTaxPaid", patterns: [
        /Total\s+Taxes?\s+Paid/i,
    ]},
    { field: "refundDue", patterns: [
        /\bRefund\b/i,
    ]},
    // "on total income" belongs to taxOnTotalIncome, not to the balance payable.
    { field: "taxPayable", patterns: [
        /Amount\s+payable/i,
        /Balance\s+Tax\s+Payable/i,
        /Tax\s+Payable(?!\s+on|\s+after)/i,
    ]},

    // ── audit rows: parsed only to close the equations, never saved ──
    { field: "capitalGainsSum", patterns: [
        /Sum\s+of\s+Short[-\s]?term\s*\/\s*Long[-\s]?term/i,
    ]},
    { field: "capitalGains115BBH", patterns: [
        /115BBH/i,
    ]},
    { field: "capitalGainsTotal", patterns: [
        /Total\s+Capital\s+Gains/i,
    ]},
    { field: "totalHeadwiseIncome", patterns: [
        /Total\s+of\s+head\s*wise\s+income/i,
    ]},
    { field: "currentYearLossSetoff", patterns: [
        /Losses\s+of\s+current\s+year\s+set\s+off/i,
    ]},
    { field: "broughtFwdLossSetoff", patterns: [
        /Brought\s+forward\s+losses\s+set\s+off/i,
    ]},
    { field: "rebate87A", patterns: [
        /Rebate\s+under\s+section\s+87A/i,
    ]},
    { field: "taxAfterRebate", patterns: [
        /Tax\s+Payable\s+after\s+rebate/i,
    ]},
    { field: "netTaxLiability", patterns: [
        /Net\s+tax\s+liability/i,
    ]},
    { field: "interestAndFee", patterns: [
        /Total\s+Interest\s+and\s+Fee\s+Payable/i,
    ]},
    { field: "aggregateLiability", patterns: [
        /Aggregate\s+liability/i,
    ]},
];

// ─── Part B invariants ───────────────────────────────────────

type NumericField = {
    [K in keyof ParsedRow]-?: ParsedRow[K] extends number | undefined ? K : never;
}[keyof ParsedRow];

/**
 * Part B arithmetic, as `total = Σ (sign × term)`.
 *
 * Every equation here holds unconditionally for ITR-2. The ones spanning income
 * heads or the tax build-up are only solvable because `ItrAuditFields` supplies
 * the loss set-off, rebate and interest rows that the saved payload omits.
 */
interface Invariant {
    equation: string;
    total: NumericField;
    terms: Array<{ field: NumericField; sign: 1 | -1 }>;
}

const INVARIANTS: Invariant[] = [
    {
        equation: "Sum of capital gains (3c) = short term + long term",
        total: "capitalGainsSum",
        terms: [
            { field: "capitalGainsStcg", sign: 1 },
            { field: "capitalGainsLtcg", sign: 1 },
        ],
    },
    {
        equation: "Total capital gains (3e) = 3c + gains taxed u/s 115BBH",
        total: "capitalGainsTotal",
        terms: [
            { field: "capitalGainsSum", sign: 1 },
            { field: "capitalGains115BBH", sign: 1 },
        ],
    },
    {
        equation: "Total head-wise income (5) = salary + house property + 3e + other sources",
        total: "totalHeadwiseIncome",
        terms: [
            { field: "salaryIncome", sign: 1 },
            { field: "housePropertyIncome", sign: 1 },
            { field: "capitalGainsTotal", sign: 1 },
            { field: "otherSourcesIncome", sign: 1 },
        ],
    },
    {
        equation: "Gross Total Income = head-wise total − current-year losses − brought-forward losses",
        total: "grossTotalIncome",
        terms: [
            { field: "totalHeadwiseIncome", sign: 1 },
            { field: "currentYearLossSetoff", sign: -1 },
            { field: "broughtFwdLossSetoff", sign: -1 },
        ],
    },
    {
        equation: "Tax payable after rebate = tax on total income − rebate u/s 87A",
        total: "taxAfterRebate",
        terms: [
            { field: "taxOnTotalIncome", sign: 1 },
            { field: "rebate87A", sign: -1 },
        ],
    },
    {
        equation: "Gross tax liability = tax after rebate + surcharge + cess",
        total: "totalTaxLiability",
        terms: [
            { field: "taxAfterRebate", sign: 1 },
            { field: "surcharge", sign: 1 },
            { field: "cess", sign: 1 },
        ],
    },
    {
        equation: "Aggregate liability = net tax liability + interest and fee",
        total: "aggregateLiability",
        terms: [
            { field: "netTaxLiability", sign: 1 },
            { field: "interestAndFee", sign: 1 },
        ],
    },
    {
        equation: "Refund = taxes paid − aggregate liability + amount still payable",
        total: "refundDue",
        terms: [
            { field: "totalTaxPaid", sign: 1 },
            { field: "aggregateLiability", sign: -1 },
            { field: "taxPayable", sign: 1 },
        ],
    },
    {
        equation: "Total Income = Gross Total Income − Chapter VI-A deductions",
        total: "totalIncome",
        terms: [
            { field: "grossTotalIncome", sign: 1 },
            { field: "chapterViaDeductions", sign: -1 },
        ],
    },
    {
        equation: "Total Taxes Paid = Advance Tax + TDS + TCS + Self-Assessment Tax",
        total: "totalTaxPaid",
        terms: [
            { field: "advanceTaxPaid", sign: 1 },
            { field: "tdsPaid", sign: 1 },
            { field: "tcsPaid", sign: 1 },
            { field: "selfAssessmentTaxPaid", sign: 1 },
        ],
    },
];

/**
 * Section 288A rounds total income to the nearest ten rupees, so an equation can
 * legitimately miss by up to 9. Anything beyond that is a misread row.
 */
const BALANCE_TOLERANCE = 10;

/**
 * Cross-checks the parsed figures against the form's own arithmetic.
 *
 * Each equation is solved as a linear constraint: fully known and balancing
 * confirms every term, fully known and off flags every term, and exactly one
 * unknown is filled in. Returns fresh objects; the inputs are not mutated.
 */
export function applyInvariants(
    data: ParsedRow,
    confidence: Record<string, FieldConfidence>,
): { data: ParsedRow; confidence: Record<string, FieldConfidence>; issues: InvariantIssue[] } {
    const out: ParsedRow = { ...data };
    const conf = { ...confidence };
    const issues: InvariantIssue[] = [];

    const num = (f: NumericField) => out[f] as number | undefined;
    // Confidence only ever moves up the rank, so two equations touching the same
    // field cannot undo each other — and a conflict always outranks a confirm.
    const raise = (f: string, level: FieldConfidence) => {
        const cur = conf[f] ?? "missing";
        if (CONFIDENCE_RANK[level] > CONFIDENCE_RANK[cur]) conf[f] = level;
    };

    // A negative or fractional figure is impossible in Part B regardless of any equation.
    for (const [field, value] of Object.entries(out)) {
        if (typeof value === "number" && (value < 0 || !Number.isFinite(value))) {
            raise(field, "conflict");
        }
    }

    for (const inv of INVARIANTS) {
        // Written as total − Σ(sign × term) = 0 so every variable has a coefficient.
        const parts: Array<{ field: NumericField; coeff: number }> = [
            { field: inv.total, coeff: 1 },
            ...inv.terms.map((t) => ({ field: t.field, coeff: -t.sign })),
        ];
        const fields = parts.map((p) => p.field);
        const unknown = parts.filter((p) => num(p.field) === undefined);

        if (unknown.length > 1) continue;

        if (unknown.length === 1) {
            const target = unknown[0];
            const known = parts
                .filter((p) => p !== target)
                .reduce((sum, p) => sum + p.coeff * num(p.field)!, 0);
            const value = -known / target.coeff;
            if (value < 0 || !Number.isInteger(value)) continue;
            (out as Record<string, unknown>)[target.field] = value;
            raise(target.field, "derived");
            continue;
        }

        const residual = parts.reduce((sum, p) => sum + p.coeff * num(p.field)!, 0);
        if (Math.abs(residual) <= BALANCE_TOLERANCE) {
            for (const f of fields) raise(f, "confirmed");
        } else {
            const expected = num(inv.total)! - residual;
            issues.push({ equation: inv.equation, expected, actual: num(inv.total)!, fields });
            for (const f of fields) raise(f, "conflict");
        }
    }

    return { data: out, confidence: conf, issues };
}

// ─── pure parsing ────────────────────────────────────────────

export function parseItrLines(lines: string[]): ItrParseResult {
    const data: ParsedRow = {};
    const confidence: Record<string, FieldConfidence> = {};
    const matchedLines: Record<string, string> = {};

    // Schedules print before Part B and repeat the same labels ("Short-term capital
    // gain", "TDS", "Surcharge"), so first-match-wins over the whole document binds
    // to a schedule row. Amounts only ever come from the Part B computation block.
    const amountLines = joinWrappedRows(partBLines(lines));

    for (const { field, patterns } of AMOUNT_RULES) {
        let value: number | undefined;
        let matched: string | undefined;
        outer: for (const pattern of patterns) {
            for (const line of amountLines) {
                if (!pattern.test(line)) continue;
                // A heading row ("b Long-term 3b") carries no amount — keep looking.
                const v = rowAmount(line);
                if (v !== undefined) {
                    value = v;
                    matched = line;
                    break outer;
                }
            }
        }
        if (value === undefined) {
            confidence[field] = "missing";
        } else {
            (data as Record<string, unknown>)[field] = value;
            confidence[field] = "parsed";
            matchedLines[field] = matched!;
        }
    }

    // Header fields
    let formType: string | null = null;
    let assessmentYear: string | null = null;
    for (const line of lines) {
        if (!formType) {
            const m = line.match(/\bITR[-\s]?([1-7])\b/i);
            if (m) formType = `ITR-${m[1]}`;
        }
        if (!assessmentYear) {
            const m = line.match(/Assessment\s+Year\s+(\d{4}\s*[-–]\s*\d{2,4})/i);
            if (m) assessmentYear = m[1].replace(/\s/g, "").replace("–", "-");
        }
        if (!data.panMasked) {
            const m = line.match(/\b[A-Z]{5}\d{4}[A-Z]\b/);
            if (m) {
                const masked = maskPan(m[0]);
                if (masked) data.panMasked = masked;
            }
        }
        if (!data.ackNumber) {
            const m = line.match(/Acknowledgement\s+Number\s*:?\s*(\d{9,20})/i);
            if (m) data.ackNumber = m[1];
        }
        if (!data.filingDate) {
            const m = line.match(/Date\s+of\s+filing\s*:?\s*([\dA-Za-z/-]+)/i);
            if (m) data.filingDate = parseDate(m[1]);
        }
        if (!data.regime) {
            const m = line.match(/115BAC[^?]*\?\s*(Yes|No)/i);
            if (m) data.regime = m[1].toLowerCase() === "yes" ? "new" : "old";
        }
    }

    for (const key of ["panMasked", "filingDate", "ackNumber", "regime"] as const) {
        confidence[key] = data[key] === undefined ? "missing" : "parsed";
    }

    const checked = applyInvariants(data, confidence);

    return {
        data: checked.data,
        confidence: checked.confidence,
        issues: checked.issues,
        rawLines: lines.slice(0, 300),
        allLines: lines,
        matchedLines,
        formType,
        assessmentYear,
    };
}

// ─── debug report ────────────────────────────────────────────

const PART_B_START = /PART\s*B\s*[–-]\s*TI\b/i;
const PART_B_END = /^\s*TAX\s+PAYMENTS\b/i;

/**
 * The Part B-TI + Part B-TTI slice of the document — every computation row the
 * parser cares about. Falls back to all lines when the section markers are absent
 * (a layout we have not seen, which is exactly when the full dump is useful).
 */
export function partBLines(lines: string[]): string[] {
    const start = lines.findIndex((l) => PART_B_START.test(l));
    if (start === -1) return lines;
    const rest = lines.slice(start);
    const end = rest.findIndex((l, i) => i > 0 && PART_B_END.test(l));
    return end === -1 ? rest : rest.slice(0, end);
}

/**
 * Plain-text report pairing every Part B raw line with what the parser made of
 * it. Written to a log file so parser rules can be tuned against a real return.
 * Contains real income and tax figures — treat as sensitive.
 */
export function buildItrDebugReport(result: ItrParseResult, fileName: string): string {
    const partB = partBLines(result.allLines);
    const out: string[] = [];

    out.push(`=== ITR parse debug — ${new Date().toISOString()} ===`);
    out.push(`file: ${fileName}`);
    out.push(`formType: ${result.formType ?? "(not found)"}`);
    out.push(`assessmentYear: ${result.assessmentYear ?? "(not found)"}`);
    out.push(`total lines: ${result.allLines.length}, Part B lines: ${partB.length}`);
    out.push("");

    out.push("--- RAW LINES: PART B-TI + PART B-TTI ---");
    partB.forEach((line, i) => out.push(`[${String(i + 1).padStart(4, "0")}] ${line}`));
    out.push("");

    out.push("--- PARSED FIELDS ---");
    const keys = Object.keys(result.confidence).sort();
    for (const key of keys) {
        const value = (result.data as Record<string, unknown>)[key];
        const level = result.confidence[key];
        if (level === "missing") {
            out.push(`${key.padEnd(24)} = MISSING`);
        } else {
            out.push(`${key.padEnd(24)} = ${String(value)} [${level}]`);
            const src = result.matchedLines[key];
            if (src) out.push(`${" ".repeat(24)}   ← "${src}"`);
        }
    }

    if (result.issues.length) {
        out.push("");
        out.push("--- FAILED INVARIANTS ---");
        for (const iss of result.issues) {
            out.push(`${iss.equation}: expected ${iss.expected}, read ${iss.actual}`);
            out.push(`${" ".repeat(4)}fields: ${iss.fields.join(", ")}`);
        }
    }

    return out.join("\n");
}

// ─── PDF text extraction ─────────────────────────────────────

async function extractLines(pdf: pdfjsLib.PDFDocumentProxy): Promise<string[]> {
    const lines: string[] = [];

    for (let pageNum = 1; pageNum <= pdf.numPages; pageNum++) {
        const page = await pdf.getPage(pageNum);
        const content = await page.getTextContent();

        // Group by y-coordinate (5 px bucket absorbs baseline noise)
        const byY = new Map<number, Array<{ x: number; str: string }>>();
        for (const raw of content.items) {
            const item = raw as TextItem;
            if (!item.str?.trim()) continue;
            const y = Math.round(item.transform[5] / 5) * 5;
            const arr = byY.get(y) ?? [];
            arr.push({ x: item.transform[4], str: item.str });
            byY.set(y, arr);
        }

        const ys = [...byY.keys()].sort((a, b) => b - a); // top-of-page first
        for (const y of ys) {
            const sorted = byY.get(y)!.sort((a, b) => a.x - b.x);
            const line = sorted.map((i) => i.str).join(" ").replace(/\s+/g, " ").trim();
            if (line) lines.push(line);
        }
    }

    return lines;
}

// ─── public API ──────────────────────────────────────────────

export async function parseItrPdf(
    buffer: ArrayBuffer,
    password?: string,
): Promise<ItrParseResult> {
    let pdf: pdfjsLib.PDFDocumentProxy;
    try {
        pdf = await pdfjsLib.getDocument({
            data: new Uint8Array(buffer),
            password: password || undefined,
        }).promise;
    } catch (e: any) {
        if (e?.name === "PasswordException") throw new ItrPasswordRequired();
        throw e;
    }

    return parseItrLines(await extractLines(pdf));
}
