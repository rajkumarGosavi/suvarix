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

export type FieldConfidence = "parsed" | "missing";

export interface ItrParseResult {
    data: ParsedItr;
    confidence: Record<string, FieldConfidence>;
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

function parseAmount(raw: string): number | undefined {
    const negative = raw.trim().startsWith("(");
    const n = parseFloat(raw.replace(/[(),]/g, ""));
    if (!Number.isFinite(n)) return undefined;
    return negative ? -n : n;
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
 * Label-anchored amount rules. The first pattern that matches any line wins, so
 * order patterns most-specific first. Capture group 1 must always be the amount.
 */
const AMOUNT_RULES: Array<{ field: keyof ParsedItr; patterns: RegExp[] }> = [
    { field: "salaryIncome", patterns: [
        new RegExp(String.raw`Salaries?\s*(?:\(.*?\))?\s+(${AMOUNT})`, "i"),
        new RegExp(String.raw`Income\s+from\s+Salary\s*\/?\s*Pension\s+(${AMOUNT})`, "i"),
    ]},
    { field: "housePropertyIncome", patterns: [
        new RegExp(String.raw`Income\s+from\s+house\s+property\s*(?:\(.*?\))?\s+(${AMOUNT})`, "i"),
    ]},
    { field: "capitalGainsStcg", patterns: [
        new RegExp(String.raw`Short[-\s]?term\s*(?:capital\s+gains?)?\s*(?:\(.*?\))?\s+(${AMOUNT})`, "i"),
    ]},
    { field: "capitalGainsLtcg", patterns: [
        new RegExp(String.raw`Long[-\s]?term\s*(?:capital\s+gains?)?\s*(?:\(.*?\))?\s+(${AMOUNT})`, "i"),
    ]},
    { field: "otherSourcesIncome", patterns: [
        new RegExp(String.raw`Income\s+from\s+other\s+sources\s*(?:\(.*?\))?\s+(${AMOUNT})`, "i"),
    ]},
    { field: "businessIncome", patterns: [
        new RegExp(String.raw`Profits?\s+and\s+gains?\s+from\s+business\s+or\s+profession\s*(?:\(.*?\))?\s+(${AMOUNT})`, "i"),
    ]},
    { field: "grossTotalIncome", patterns: [
        new RegExp(String.raw`Gross\s+Total\s+Income\s*(?:\(.*?\))?\s+(${AMOUNT})`, "i"),
    ]},
    { field: "chapterViaDeductions", patterns: [
        new RegExp(String.raw`Deductions?\s+under\s+Chapter\s+VI[-\s]?A\s*(?:\(.*?\))?\s+(${AMOUNT})`, "i"),
        new RegExp(String.raw`Total\s+Deductions?\s+under\s+Chapter\s+VI[-\s]?A\s+(${AMOUNT})`, "i"),
    ]},
    // Lookbehind keeps this off the "Gross Total Income" line, which appears first.
    { field: "totalIncome", patterns: [
        new RegExp(String.raw`(?<!Gross\s)Total\s+Income\s*(?:\(.*?\))?\s+(${AMOUNT})`, "i"),
    ]},
    { field: "taxOnTotalIncome", patterns: [
        new RegExp(String.raw`Tax\s+payable\s+on\s+total\s+income\s*(?:\(.*?\))?\s+(${AMOUNT})`, "i"),
    ]},
    { field: "surcharge", patterns: [
        new RegExp(String.raw`Surcharge\s*(?:\(.*?\))?\s+(${AMOUNT})`, "i"),
    ]},
    { field: "cess", patterns: [
        new RegExp(String.raw`(?:Health\s+and\s+Education\s+)?Cess\s*(?:\(.*?\))?\s+(${AMOUNT})`, "i"),
    ]},
    { field: "totalTaxLiability", patterns: [
        new RegExp(String.raw`Gross\s+tax\s+liability\s*(?:\(.*?\))?\s+(${AMOUNT})`, "i"),
        new RegExp(String.raw`Total\s+Tax\s+(?:and\s+Interest\s+)?Liability\s+(${AMOUNT})`, "i"),
    ]},
    { field: "tdsPaid", patterns: [
        new RegExp(String.raw`\bTDS\b\s*(?:\(.*?\))?\s+(${AMOUNT})`, "i"),
    ]},
    { field: "advanceTaxPaid", patterns: [
        new RegExp(String.raw`Advance\s+Tax\s*(?:\(.*?\))?\s+(${AMOUNT})`, "i"),
    ]},
    { field: "selfAssessmentTaxPaid", patterns: [
        new RegExp(String.raw`Self[-\s]?Assessment\s+Tax\s*(?:\(.*?\))?\s+(${AMOUNT})`, "i"),
    ]},
    { field: "tcsPaid", patterns: [
        new RegExp(String.raw`\bTCS\b\s*(?:\(.*?\))?\s+(${AMOUNT})`, "i"),
    ]},
    { field: "totalTaxPaid", patterns: [
        new RegExp(String.raw`Total\s+Taxes?\s+Paid\s*(?:\(.*?\))?\s+(${AMOUNT})`, "i"),
    ]},
    { field: "refundDue", patterns: [
        new RegExp(String.raw`Refund\s*(?:\(.*?\))?\s+(${AMOUNT})`, "i"),
    ]},
    { field: "taxPayable", patterns: [
        new RegExp(String.raw`(?:Amount\s+payable|Tax\s+Payable)\s*(?:\(.*?\))?\s+(${AMOUNT})`, "i"),
    ]},
];

// ─── pure parsing ────────────────────────────────────────────

export function parseItrLines(lines: string[]): ItrParseResult {
    const data: ParsedItr = {};
    const confidence: Record<string, FieldConfidence> = {};
    const matchedLines: Record<string, string> = {};

    for (const { field, patterns } of AMOUNT_RULES) {
        let value: number | undefined;
        let matched: string | undefined;
        outer: for (const pattern of patterns) {
            for (const line of lines) {
                const m = line.match(pattern);
                if (m) {
                    value = parseAmount(m[1]);
                    if (value !== undefined) {
                        matched = line;
                        break outer;
                    }
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

    return {
        data,
        confidence,
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
        if (result.confidence[key] === "missing") {
            out.push(`${key.padEnd(24)} = MISSING`);
        } else {
            out.push(`${key.padEnd(24)} = ${String(value)}`);
            const src = result.matchedLines[key];
            if (src) out.push(`${" ".repeat(24)}   ← "${src}"`);
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
