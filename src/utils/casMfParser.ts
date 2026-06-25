import * as pdfjsLib from "pdfjs-dist";
import type { TextItem } from "pdfjs-dist/types/src/display/api";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";

pdfjsLib.GlobalWorkerOptions.workerSrc = workerUrl;

export interface CasMfHolding {
    schemeName: string;
    isin: string;
    folioNumber: string;
    units: number;
    nav: number;
    amcName: string;
    isDirect: boolean;
    isGrowth: boolean;
}

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
    return "Unknown AMC";
}

async function extractLines(pdf: pdfjsLib.PDFDocumentProxy): Promise<string[]> {
    const lines: string[] = [];

    for (let pageNum = 1; pageNum <= pdf.numPages; pageNum++) {
        const page = await pdf.getPage(pageNum);
        const content = await page.getTextContent();

        // Group text items by y-coordinate (bucket to 2px for alignment noise)
        const byY = new Map<number, Array<{ x: number; str: string }>>();
        for (const raw of content.items) {
            const item = raw as TextItem;
            if (!item.str?.trim()) continue;
            const y = Math.round(item.transform[5] / 2) * 2;
            const arr = byY.get(y) ?? [];
            arr.push({ x: item.transform[4], str: item.str });
            byY.set(y, arr);
        }

        // Sort y descending (top of page first), then x ascending within each line
        const ys = [...byY.keys()].sort((a, b) => b - a);
        for (const y of ys) {
            const sorted = byY.get(y)!.sort((a, b) => a.x - b.x);
            const line = sorted.map((i) => i.str).join(" ").replace(/\s+/g, " ").trim();
            if (line) lines.push(line);
        }
    }

    return lines;
}

export async function parseCasPdf(
    buffer: ArrayBuffer,
    password: string,
): Promise<CasMfHolding[]> {
    const loadingTask = pdfjsLib.getDocument({
        data: new Uint8Array(buffer),
        password: password || undefined,
    });

    const pdf = await loadingTask.promise;
    const lines = await extractLines(pdf);

    const holdings: CasMfHolding[] = [];

    // State
    let folio = "";
    let schemeName = "";
    let isin = "";
    let closingUnits: number | null = null;
    let prevLine = "";

    const folioRe = /Folio\s*(?:No\.?)?\s*[:\-]\s*([\w\/]+)/i;
    const isinRe = /ISIN\s*[:\-]\s*(IN[A-Z0-9]{10})/i;
    // "Closing Balance (dd-Mon-yyyy)  123.456" or "Closing Balance : 123.456 Units"
    const closingRe = /Closing Balance[^0-9]+([\d,]+\.\d{3,})/i;
    // NAV line: "NAV on ...: Rs. 123.45" or "Rs. 123.45"
    const navLineRe = /NAV[^0-9]+Rs\.?\s*([\d,]+\.\d+)/i;
    const navInlineRe = /Rs\.?\s*([\d,]+\.\d+)/i;
    // Skip lines that are clearly not scheme names
    const skipLineRe = /Folio|KYC|PAN\s*:|CAMS|KFin|Franklin Templeton Registrar|Registrar|Advisor|Nominee|Tax|Statement|Portfolio|Page\s+\d/i;

    for (const line of lines) {
        // Folio
        const folioMatch = line.match(folioRe);
        if (folioMatch) {
            folio = folioMatch[1].trim();
            schemeName = "";
            isin = "";
            closingUnits = null;
            prevLine = line;
            continue;
        }

        // ISIN — line just before (prevLine) is the scheme name
        const isinMatch = line.match(isinRe);
        if (isinMatch) {
            isin = isinMatch[1];
            if (prevLine && !skipLineRe.test(prevLine)) {
                schemeName = prevLine.trim();
            }
            closingUnits = null;
            prevLine = line;
            continue;
        }

        // Closing balance — captures units
        const closingMatch = line.match(closingRe);
        if (closingMatch && isin && folio) {
            closingUnits = parseNum(closingMatch[1]);

            // NAV might be on the same line (some CAS formats)
            const navSame = line.match(navLineRe);
            if (navSame) {
                const nav = parseNum(navSame[1]);
                if (closingUnits > 0 && nav > 0 && schemeName && isin) {
                    holdings.push({
                        schemeName,
                        isin,
                        folioNumber: folio,
                        units: closingUnits,
                        nav,
                        amcName: detectAmc(schemeName),
                        isDirect: /direct/i.test(schemeName),
                        isGrowth: /growth/i.test(schemeName),
                    });
                }
                closingUnits = null;
                isin = "";
                schemeName = "";
            }
            prevLine = line;
            continue;
        }

        // NAV line following a closing balance
        if (closingUnits !== null && isin && folio) {
            const navMatch = line.match(navLineRe) ?? line.match(navInlineRe);
            if (navMatch) {
                const nav = parseNum(navMatch[1]);
                if (closingUnits > 0 && nav > 0 && schemeName && isin) {
                    holdings.push({
                        schemeName,
                        isin,
                        folioNumber: folio,
                        units: closingUnits,
                        nav,
                        amcName: detectAmc(schemeName),
                        isDirect: /direct/i.test(schemeName),
                        isGrowth: /growth/i.test(schemeName),
                    });
                }
                closingUnits = null;
                isin = "";
                schemeName = "";
            }
        }

        prevLine = line;
    }

    return holdings;
}
