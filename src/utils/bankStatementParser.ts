// Parser for HDFC / ICICI bank account statements (XLSX + CSV/delimited).
//
// Convention (matches casMfParser.ts / parseZerodhaXlsx): the frontend parses
// the file into normalized rows; Rust (`import_bank_statement`) validates and
// inserts. All bank-specific column quirks live here.
//
// Both formats are reduced to a `string[][]` grid first, then a single shared
// pipeline runs: locate the header row (statements have page-title / account /
// address / "***" preamble before it), map columns by alias, and map data rows.

import * as XLSX from "xlsx";
import { categorize } from "./txnCategorize";

export type BankId = "hdfc" | "icici";

export interface BankTxnRow {
    date: string;        // normalized "YYYY-MM-DD"
    narration: string;
    refNo?: string;
    amount: number;      // signed: +credit (income), -debit (expense)
    type: "income" | "expense";
    category: string;
    tag: string;         // bank display name, e.g. "HDFC"
    include: boolean;    // preview checkbox — user can exclude before import
}

export interface ParseResult {
    rows: BankTxnRow[];
    // Raw grid rows exposed for debugging when nothing matched (mirrors
    // casMfParser's rawLines) — lets the UI show why a layout failed.
    rawGrid: string[][];
    headerRowIndex: number;
}

const BANK_LABEL: Record<BankId, string> = { hdfc: "HDFC", icici: "ICICI" };

// Column alias lists (matched after header normalization below). Cover both
// HDFC and ICICI headers so one detector handles both banks.
const ALIASES = {
    date: ["date", "transaction_date", "txn_date", "value_date", "value_dt", "tran_date"],
    narration: ["narration", "transaction_remarks", "description", "particulars", "remarks", "transaction_details"],
    ref: ["chq_ref_no", "cheque_no", "ref_no", "reference_no", "chq_no", "cheque_number", "ref_cheque_no"],
    withdrawal: ["withdrawal_amt", "withdrawal_amount_inr", "withdrawal_amount", "withdrawals", "withdrawal", "debit", "debit_amount", "dr"],
    deposit: ["deposit_amt", "deposit_amount_inr", "deposit_amount", "deposits", "deposit", "credit", "credit_amount", "cr"],
} as const;

/** Normalize a header cell to snake_case token, e.g. "Withdrawal Amt." → "withdrawal_amt". */
function normHeader(h: unknown): string {
    return String(h ?? "")
        .trim()
        .toLowerCase()
        .replace(/[^a-z0-9]/g, "_")
        .replace(/_+/g, "_")
        .replace(/^_+|_+$/g, "");
}

/** Index of the first alias present in the normalized header row, else -1. */
function findCol(headers: string[], names: readonly string[]): number {
    for (const n of names) {
        const i = headers.indexOf(n);
        if (i >= 0) return i;
    }
    return -1;
}

/** Parse "DD/MM/YY" or "DD/MM/YYYY" (also "-" separators) → "YYYY-MM-DD", else null. */
function normDate(raw: string): string | null {
    const s = (raw || "").trim();
    const m = s.match(/^(\d{1,2})[\/\-.](\d{1,2})[\/\-.](\d{2}|\d{4})$/);
    if (!m) return null;
    const day = m[1].padStart(2, "0");
    const month = m[2].padStart(2, "0");
    let year = m[3];
    if (year.length === 2) year = `20${year}`;
    const d = Number(day), mo = Number(month);
    if (mo < 1 || mo > 12 || d < 1 || d > 31) return null;
    return `${year}-${month}-${day}`;
}

/** Parse an Indian-formatted amount cell ("1,234.50") → number, 0 if blank/unparseable. */
function parseAmount(raw: unknown): number {
    const s = String(raw ?? "").replace(/[,\s₹]/g, "").trim();
    if (!s) return 0;
    const n = Number(s);
    return Number.isFinite(n) ? n : 0;
}

/** Build a string grid from CSV/delimited text, auto-detecting tab vs comma. */
function csvToGrid(text: string): string[][] {
    const lines = text.split(/\r?\n/);
    // Detect delimiter from the line with the most separators (HDFC delimited = tab).
    let tabs = 0, commas = 0;
    for (const l of lines.slice(0, 60)) {
        tabs += (l.match(/\t/g) || []).length;
        commas += (l.match(/,/g) || []).length;
    }
    const delim = tabs >= commas ? "\t" : ",";
    return lines.map((line) =>
        line.split(delim).map((c) => c.trim().replace(/^"|"$/g, "").trim()),
    );
}

/** Build a string grid from an XLSX buffer (first sheet). */
function xlsxToGrid(buffer: ArrayBuffer): string[][] {
    const wb = XLSX.read(new Uint8Array(buffer), { type: "array" });
    const first = wb.SheetNames[0];
    const ws = first ? wb.Sheets[first] : undefined;
    if (!ws) return [];
    const rows: unknown[][] = XLSX.utils.sheet_to_json(ws, { header: 1, defval: "" });
    return rows.map((r) => r.map((c) => String(c ?? "").trim()));
}

/** True if a normalized header row contains both a date and a narration column. */
function isHeaderRow(normalized: string[]): boolean {
    return findCol(normalized, ALIASES.date) >= 0 && findCol(normalized, ALIASES.narration) >= 0;
}

/**
 * Parse a bank statement file into normalized transaction rows.
 * `bank` selects the display tag; column detection itself is bank-agnostic.
 */
export function parseBankStatement(bank: BankId, fileName: string, buffer: ArrayBuffer): ParseResult {
    const isCsv = /\.(csv|txt)$/i.test(fileName);
    const grid = isCsv ? csvToGrid(new TextDecoder("utf-8").decode(buffer)) : xlsxToGrid(buffer);

    // Locate the header row (skips page-title / account / address / "***" preamble).
    let headerRowIndex = -1;
    let headers: string[] = [];
    for (let i = 0; i < grid.length; i++) {
        const norm = grid[i].map(normHeader);
        if (isHeaderRow(norm)) {
            headerRowIndex = i;
            headers = norm;
            break;
        }
    }
    if (headerRowIndex < 0) {
        return { rows: [], rawGrid: grid.slice(0, 40), headerRowIndex: -1 };
    }

    const dateIdx = findCol(headers, ALIASES.date);
    const narrIdx = findCol(headers, ALIASES.narration);
    const refIdx = findCol(headers, ALIASES.ref);
    const wdlIdx = findCol(headers, ALIASES.withdrawal);
    const depIdx = findCol(headers, ALIASES.deposit);

    const tag = BANK_LABEL[bank];
    const rows: BankTxnRow[] = [];

    for (let i = headerRowIndex + 1; i < grid.length; i++) {
        const cells = grid[i];
        // Stop / skip separator, blank, and footer rows.
        const joined = cells.join("").trim();
        if (!joined || /^\*+$/.test(joined)) continue;

        const date = dateIdx >= 0 ? normDate(cells[dateIdx] ?? "") : null;
        if (!date) continue; // drops totals / summary / wrapped rows with no valid date

        const withdrawal = wdlIdx >= 0 ? parseAmount(cells[wdlIdx]) : 0;
        const deposit = depIdx >= 0 ? parseAmount(cells[depIdx]) : 0;
        const signed = deposit - withdrawal;
        if (signed === 0) continue; // no money moved (or unparseable amounts)

        const narration = narrIdx >= 0 ? String(cells[narrIdx] ?? "").trim() : "";
        const refNo = refIdx >= 0 ? String(cells[refIdx] ?? "").trim() || undefined : undefined;

        rows.push({
            date,
            narration,
            refNo,
            amount: signed,
            type: signed < 0 ? "expense" : "income",
            category: categorize(narration),
            tag,
            include: true,
        });
    }

    return { rows, rawGrid: grid.slice(0, 40), headerRowIndex };
}
