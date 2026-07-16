// Heuristic parser for OCR'd Indian retail receipts (ML Kit text output).
// Best-effort only — fields the heuristics can't find come back null and the
// user reviews everything in the prefilled Add Transaction dialog before save.
import { categorize } from "./txnCategorize";

export interface OcrLine {
    text: string;
    top: number;
}

export interface ParsedReceipt {
    merchant: string | null;
    date: Date | null;
    amount: number | null;
    category: string;
}

// ── amount ──────────────────────────────────────────────────────────

// Ordered high → low priority; first keyword tier with a value wins.
const AMOUNT_KEYWORDS: RegExp[] = [
    /GRAND\s*TOTAL/i,
    /NET\s*(?:PAYABLE|AMOUNT|TOTAL)/i,
    /AMOUNT\s*(?:PAID|PAYABLE|DUE)/i,
    /BILL\s*AMOUNT/i,
    /\bTOTAL\b/i,
];
const NOT_A_TOTAL = /SUB\s*TOTAL|TOTAL\s*(?:ITEMS?|QTY|QUANTITY|SAVINGS)/i;
// Lines whose numbers are ids/contacts, never amounts.
const NOISE_LINE =
    /GSTIN|FSSAI|PH(?:ONE)?\b|\bMOB\b|MOBILE|PIN\s*CODE|PINCODE|BILL\s*NO|INVOICE\s*NO|RECEIPT\s*NO|ORDER\s*NO|CASH\s*TENDERED|CHANGE\s*DUE|\bTIN\b/i;

const MONEY_TOKEN = /(₹|RS\.?|INR)?\s*(\d(?:[\d,]*\d)?(?:\.\d{1,2})?)/gi;

interface MoneyCandidate {
    value: number;
    hasDecimals: boolean;
    hasCurrency: boolean;
}

// Remove dates, times, and percentages so their digits can't read as amounts.
function stripNonAmounts(line: string): string {
    return line
        .replace(/\b\d{4}-\d{2}-\d{2}\b/g, " ")
        .replace(/\b\d{1,2}[/\-.]\d{1,2}[/\-.]\d{2,4}\b/g, " ")
        .replace(/\b\d{1,2}:\d{2}(?::\d{2})?\b/g, " ")
        .replace(/\d+(?:\.\d+)?\s*%/g, " ");
}

function extractMoney(line: string): MoneyCandidate[] {
    const out: MoneyCandidate[] = [];
    for (const m of stripNonAmounts(line).matchAll(MONEY_TOKEN)) {
        const hasCurrency = !!m[1];
        const raw = m[2];
        if (!raw) continue;
        const hasDecimals = /\.\d{1,2}$/.test(raw);
        const digits = raw.replace(/\D/g, "");
        if (digits.length > 9) continue; // phone numbers / ids, never a receipt amount
        const value = parseFloat(raw.replace(/,/g, ""));
        if (!Number.isFinite(value) || value <= 0) continue;
        out.push({ value, hasDecimals, hasCurrency });
    }
    return out;
}

// A line that is just a number (with optional currency marker) — OCR often
// splits "TOTAL" and its value onto separate lines.
function isBareAmountLine(line: string): boolean {
    const rest = line.replace(/₹|RS\.?|INR/gi, "");
    return /\d/.test(rest) && !/[A-Za-z]/.test(rest);
}

function findAmount(lines: string[]): number | null {
    for (const kw of AMOUNT_KEYWORDS) {
        let best: number | null = null;
        for (let i = 0; i < lines.length; i++) {
            if (!kw.test(lines[i]) || NOT_A_TOTAL.test(lines[i])) continue;
            let cands = extractMoney(lines[i]);
            if (!cands.length && i + 1 < lines.length && isBareAmountLine(lines[i + 1])) {
                cands = extractMoney(lines[i + 1]);
            }
            for (const c of cands) best = Math.max(best ?? 0, c.value);
        }
        if (best != null) return best;
    }
    // No keyword hit: largest money-looking value, decimals/currency first.
    const withDecimals: number[] = [];
    const bareInts: number[] = [];
    for (const line of lines) {
        if (NOISE_LINE.test(line)) continue;
        for (const c of extractMoney(line)) {
            if (c.hasDecimals || c.hasCurrency) withDecimals.push(c.value);
            else if (c.value >= 10 && c.value < 10_000_000) bareInts.push(c.value);
        }
    }
    if (withDecimals.length) return Math.max(...withDecimals);
    if (bareInts.length) return Math.max(...bareInts);
    return null;
}

// ── date ────────────────────────────────────────────────────────────

const MONTHS: Record<string, number> = {
    JAN: 0, FEB: 1, MAR: 2, APR: 3, MAY: 4, JUN: 5,
    JUL: 6, AUG: 7, SEP: 8, OCT: 9, NOV: 10, DEC: 11,
};

function buildDate(year: number, month: number, day: number): Date | null {
    if (year < 100) year += 2000;
    const now = new Date();
    if (year < 2000 || year > now.getFullYear()) return null;
    if (month < 0 || month > 11 || day < 1 || day > 31) return null;
    const date = new Date(year, month, day);
    if (date.getMonth() !== month || date.getDate() !== day) return null; // e.g. 31/02
    if (date.getTime() > now.getTime() + 24 * 3600 * 1000) return null; // future
    return date;
}

function matchDate(line: string): Date | null {
    let m = line.match(/\b(\d{4})-(\d{2})-(\d{2})\b/);
    if (m) {
        const d = buildDate(+m[1], +m[2] - 1, +m[3]);
        if (d) return d;
    }
    // Indian convention: day first.
    m = line.match(/\b(\d{1,2})[/\-.](\d{1,2})[/\-.](\d{2,4})\b/);
    if (m) {
        const d = buildDate(+m[3], +m[2] - 1, +m[1]);
        if (d) return d;
    }
    m = line.match(/\b(\d{1,2})[\s\-]*(JAN|FEB|MAR|APR|MAY|JUN|JUL|AUG|SEP|OCT|NOV|DEC)[A-Z]*[\s\-,]+(\d{2,4})\b/i);
    if (m) {
        const d = buildDate(+m[3], MONTHS[m[2].toUpperCase()], +m[1]);
        if (d) return d;
    }
    return null;
}

function findDate(lines: string[]): Date | null {
    const preferred: string[] = [];
    const rest: string[] = [];
    for (const l of lines) (/\b(?:DATE|DT|BILL)\b/i.test(l) ? preferred : rest).push(l);
    for (const l of [...preferred, ...rest]) {
        const d = matchDate(l);
        if (d) return d;
    }
    return null;
}

// ── merchant ────────────────────────────────────────────────────────

const MERCHANT_SKIP =
    /TAX\s*INVOICE|RETAIL\s*INVOICE|\bINVOICE\b|\bRECEIPT\b|CASH\s*MEMO|GSTIN|FSSAI|WELCOME|THANK/i;

function findMerchant(lines: string[]): string | null {
    for (const line of lines.slice(0, 5)) {
        if (line.length < 3) continue;
        const letters = (line.match(/[A-Za-z]/g) ?? []).length;
        if (letters < 3) continue;
        const digits = (line.match(/\d/g) ?? []).length;
        if (digits / line.length > 0.4) continue;
        if (MERCHANT_SKIP.test(line)) continue;
        if (matchDate(line)) continue;
        return line;
    }
    return null;
}

// ── entry point ─────────────────────────────────────────────────────

export function parseReceipt(fullText: string, ocrLines?: OcrLine[]): ParsedReceipt {
    const lines = (ocrLines?.length ? ocrLines.map((l) => l.text) : fullText.split(/\r?\n/))
        .map((l) => l.trim())
        .filter(Boolean);

    const merchant = findMerchant(lines);
    return {
        merchant,
        date: findDate(lines),
        amount: findAmount(lines),
        category: categorize([merchant ?? "", fullText].join("\n")),
    };
}
