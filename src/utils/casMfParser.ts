import * as pdfjsLib from "pdfjs-dist";
import type { TextItem } from "pdfjs-dist/types/src/display/api";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";

pdfjsLib.GlobalWorkerOptions.workerSrc = workerUrl;

export interface CasMfHolding {
    schemeName: string;
    isin: string;
    folioNumber: string;
    units: number;
    nav: number;      // current NAV as of CAS date
    avgNav: number;   // purchase cost basis; 0 when unavailable (detailed format)
    amcName: string;
    isDirect: boolean;
    isGrowth: boolean;
}

export interface CasParseResult {
    holdings: CasMfHolding[];
    rawLines: string[];
    format: "detailed" | "summary" | "unknown";
}

// ─── helpers ────────────────────────────────────────────────

function parseNum(s: string): number {
    return parseFloat(s.replace(/,/g, "")) || 0;
}

function detectAmc(schemeName: string): string {
    const n = schemeName.toLowerCase();
    if (n.includes("hdfc")) return "HDFC AMC";
    if (n.includes("sbi")) return "SBI Funds";
    if (n.includes("icici") || n.includes("prudential")) return "ICICI Prudential AMC";
    if (n.includes("axis")) return "Axis AMC";
    if (n.includes("kotak")) return "Kotak Mahindra AMC";
    if (n.includes("nippon") || n.includes("reliance")) return "Nippon India AMC";
    if (n.includes("mirae")) return "Mirae Asset AMC";
    if (n.includes("parag parikh") || n.includes("ppfas")) return "PPFAS AMC";
    if (n.includes("franklin")) return "Franklin Templeton";
    if (n.includes("aditya birla") || n.includes("absl")) return "Aditya Birla Sun Life AMC";
    if (n.includes("tata")) return "Tata AMC";
    if (n.includes("uti")) return "UTI AMC";
    if (n.includes("dsp")) return "DSP AMC";
    if (n.includes("invesco")) return "Invesco AMC";
    if (n.includes("quant")) return "Quant AMC";
    if (n.includes("motilal")) return "Motilal Oswal AMC";
    if (n.includes("navi")) return "Navi AMC";
    if (n.includes("zerodha")) return "Zerodha Fund House";
    if (n.includes("edelweiss")) return "Edelweiss AMC";
    if (n.includes("pgim")) return "PGIM India AMC";
    if (n.includes("lic")) return "LIC MF";
    if (n.includes("hsbc")) return "HSBC AMC";
    if (n.includes("360 one") || n.includes("360one")) return "360 ONE AMC";
    if (n.includes("sundaram")) return "Sundaram AMC";
    if (n.includes("bandhan")) return "Bandhan AMC";
    return "Unknown AMC";
}

function makeHolding(
    schemeName: string,
    isin: string,
    folioNumber: string,
    units: number,
    nav: number,
    avgNav = 0,
): CasMfHolding {
    const name = schemeName.trim();
    return {
        schemeName: name,
        isin,
        folioNumber,
        units,
        nav,
        avgNav,
        amcName: detectAmc(name),
        isDirect: /direct/i.test(name),
        isGrowth: /growth/i.test(name),
    };
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

// ─── Detailed CAS format ─────────────────────────────────────
//
// Structure per holding:
//   FOLIO NO: 1040619763 KYC : OK
//   [Scheme Name] (Advisor: ...) ISIN: INF209K01VA3 Opening Unit Balance: 20.172
//   [optional transaction rows]
//   Closing Unit Balance: 20.172 Nav as on 24-JUN-2026: INR 452.6016 Valuation ...
//
// ISIN is always on the scheme description line (same line as the scheme name).

function parseDetailedLines(lines: string[]): CasMfHolding[] {
    const holdings: CasMfHolding[] = [];

    let folio = "";
    let schemeName = "";
    let isin = "";

    const folioRe = /FOLIO\s+NO\s*:\s*(\d+)/i;
    const isinRe  = /ISIN\s*:\s*(IN[A-Z0-9]{10})/i;
    // Closing Unit Balance: 20.172 Nav as on 24-JUN-2026: INR 452.6016 Valuation ...
    const closingRe = /Closing\s+Unit\s+Balance\s*:\s*([\d,]+\.\d+).*?INR\s+([\d,]+\.\d+)\s+Valuation/i;

    for (const line of lines) {
        // New folio
        const folioMatch = line.match(folioRe);
        if (folioMatch) {
            folio = folioMatch[1];
            schemeName = "";
            isin = "";
            continue;
        }

        // Scheme description line — contains ISIN
        if (folio && !isin) {
            const isinMatch = line.match(isinRe);
            if (isinMatch) {
                isin = isinMatch[1];

                // Scheme name = everything before "(Advisor:" (or "(Advisor ")
                const advisorIdx = line.search(/\s*\(Advisor[:\s]/i);
                if (advisorIdx > 0) {
                    schemeName = line.substring(0, advisorIdx).trim();
                } else {
                    // Fallback: strip everything from " ISIN:" onwards
                    schemeName = line.replace(/\s+ISIN\s*:.*/, "").trim();
                }
                // Strip trailing "(formerly ...)" clause
                schemeName = schemeName.replace(/\s*\(formerly\b.*$/i, "").trim();
            }
        }

        // Closing balance line
        if (folio && isin) {
            const closingMatch = line.match(closingRe);
            if (closingMatch) {
                const units = parseNum(closingMatch[1]);
                const nav   = parseNum(closingMatch[2]);
                if (units > 0) {
                    holdings.push(makeHolding(schemeName, isin, folio, units, nav));
                }
                // Reset scheme state; keep folio (same folio may have multiple schemes)
                isin = "";
                schemeName = "";
            }
        }
    }

    return holdings;
}

// ─── Summary CAS format ──────────────────────────────────────
//
// Each table row (one data line, keyed by dd-Mon-yyyy date):
//
//   Pattern A (name fits on one row):
//     [folio] [scheme_name] [invested] [units] [dd-Mon-yyyy] [nav] [market_value]
//
//   Pattern B (name wraps):
//     [scheme_name_part1] [gain_abs]                 ← previous line
//     [folio] [invested] [units] [dd-Mon-yyyy] [nav] [market_value]
//     (+gain%)
//     [scheme_name_part2]                            ← continuation 2 lines after folio row
//
// No ISINs in this format.

function isSkipLineSummary(line: string): boolean {
    if (/^\(/.test(line)) return true;
    if (/^[\d,]+\.\d{1,2}$/.test(line.trim())) return true;
    if (/MFCentral|Page\s+\d/i.test(line)) return true;
    if (/Consolidated Account|SoA Holdings|Demat Holdings/i.test(line)) return true;
    if (/^Folio No\.|^Client Id|^\(INR\)|^Invested Value|^NAV Date/i.test(line)) return true;
    if (/^Total\s+[\d,]/i.test(line)) return true;
    if (/No MF holdings/i.test(line)) return true;
    return false;
}

function parseSummaryLines(lines: string[]): CasMfHolding[] {
    const holdings: CasMfHolding[] = [];

    // money: exactly 2dp  |  units: exactly 3dp  |  nav: 2–4dp  |  date
    const M = "([\\d,]+\\.\\d{2}(?!\\d))";
    const U = "([\\d,]+\\.\\d{3}(?!\\d))";
    const N = "([\\d,]+\\.\\d{2,4}(?!\\d))";
    const D = "(\\d{2}-[A-Za-z]{3}-\\d{4})";
    const F = "(\\d{5,})";

    // Pattern A: folio + scheme_name (text) + M + U + D + N + M
    const FULL = new RegExp(`^${F}\\s+(.+?)\\s+${M}\\s+${U}\\s+${D}\\s+${N}\\s+${M}`);
    // Pattern B: folio + M + U + D + N + M  (no scheme name embedded)
    const BARE = new RegExp(`^${F}\\s+${M}\\s+${U}\\s+${D}\\s+${N}\\s+${M}`);

    let lastSchemeLine = "";

    for (let i = 0; i < lines.length; i++) {
        const line = lines[i];

        // Pattern A — scheme name embedded
        const fullMatch = line.match(FULL);
        if (fullMatch) {
            // groups: folio, schemeName, invested, units, date, nav, marketValue
            const [, folio, schemeName, invested, units, , nav] = fullMatch;
            const u = parseNum(units);
            if (u > 0) {
                const inv = parseNum(invested);
                const avgNav = inv > 0 ? inv / u : parseNum(nav);
                holdings.push(makeHolding(schemeName, "", folio, u, parseNum(nav), avgNav));
            }
            lastSchemeLine = "";
            continue;
        }

        // Pattern B — scheme name from previous line(s)
        const bareMatch = line.match(BARE);
        if (bareMatch) {
            // groups: folio, invested, units, date, nav, marketValue
            const [, folio, invested, units, , nav] = bareMatch;
            let name = lastSchemeLine;
            // Check i+2 for a scheme continuation (line i+1 is the gain% line)
            if (i + 2 < lines.length) {
                const cont = lines[i + 2];
                if (cont && /^[A-Za-z]/.test(cont) && !isSkipLineSummary(cont)) {
                    name = (name + " " + cont).trim();
                }
            }
            const u = parseNum(units);
            if (u > 0 && name) {
                const inv = parseNum(invested);
                const avgNav = inv > 0 ? inv / u : parseNum(nav);
                holdings.push(makeHolding(name, "", folio, u, parseNum(nav), avgNav));
            }
            lastSchemeLine = "";
            continue;
        }

        // Track potential scheme-name prefix lines
        if (/^[A-Za-z]/.test(line) && !isSkipLineSummary(line)) {
            // Strip trailing gain/loss amount mixed into the same visual row
            const cleaned = line.replace(/\s+[\d,]+\.\d{2}\s*$/, "").trim();
            if (cleaned) lastSchemeLine = cleaned;
        }
    }

    return holdings;
}

// ─── Auto-detect format ───────────────────────────────────────

function detectFormat(lines: string[]): "detailed" | "summary" {
    const hasDetailedMarkers =
        lines.some((l) => /FOLIO\s+NO\s*:/i.test(l)) &&
        lines.some((l) => /Closing\s+Unit\s+Balance\s*:/i.test(l));
    return hasDetailedMarkers ? "detailed" : "summary";
}

// ─── Merge summary + detailed ────────────────────────────────
//
// Summary provides: avgNav (from invested/units), no ISIN
// Detailed provides: ISIN, no avg cost basis
// Merge by (folio + normalised scheme name) to get both on each holding.

function normaliseName(name: string): string {
    return name.toLowerCase().replace(/\s+/g, " ").trim();
}

export function mergeCasHoldings(
    summary: CasMfHolding[],
    detailed: CasMfHolding[],
): CasMfHolding[] {
    // Exact match: folio + name → isin
    const exactMap = new Map<string, string>();
    // Fallback: folio → [isin, ...] — use when folio has only one scheme in detailed
    const folioMap = new Map<string, string[]>();

    for (const h of detailed) {
        const key = `${h.folioNumber}::${normaliseName(h.schemeName)}`;
        exactMap.set(key, h.isin);
        const arr = folioMap.get(h.folioNumber) ?? [];
        arr.push(h.isin);
        folioMap.set(h.folioNumber, arr);
    }

    return summary.map((h) => {
        const key = `${h.folioNumber}::${normaliseName(h.schemeName)}`;
        let isin = exactMap.get(key) ?? "";
        if (!isin) {
            const candidates = folioMap.get(h.folioNumber) ?? [];
            if (candidates.length === 1) isin = candidates[0];
        }
        return { ...h, isin };
    });
}

// ─── public API ──────────────────────────────────────────────

export async function parseCasPdf(
    buffer: ArrayBuffer,
    password: string,
): Promise<CasParseResult> {
    const loadingTask = pdfjsLib.getDocument({
        data: new Uint8Array(buffer),
        password: password || undefined,
    });

    const pdf = await loadingTask.promise;
    const lines = await extractLines(pdf);

    const format = detectFormat(lines);
    const holdings =
        format === "detailed"
            ? parseDetailedLines(lines)
            : parseSummaryLines(lines);

    return { holdings, rawLines: lines.slice(0, 300), format };
}
