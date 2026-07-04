<script setup lang="ts">
import { ref, onMounted, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import * as XLSX from "xlsx";
import { usePricesStore } from "@/stores/prices";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";
import { useZerodhaStore } from "@/stores/zerodha";
import { useUpstoxStore } from "@/stores/upstox";
import { useAngelOneStore } from "@/stores/angel_one";
import { parseCasPdf, mergeCasHoldings, type CasMfHolding } from "@/utils/casMfParser";
import { useAnalytics } from "@/composables/useAnalytics";
import { useToast } from "primevue/usetoast";

const store = usePricesStore();
const zerodha = useZerodhaStore();
const upstox = useUpstoxStore();
const angelOne = useAngelOneStore();
const { track } = useAnalytics();
const toast = useToast();
const { formatINR } = useCurrencyFormat();

// Zerodha & Upstox use localhost TCP OAuth — not possible on Android
const isAndroid = /android/i.test(navigator.userAgent);

// ── Zerodha inputs ──────────────────────────────────────────────
const apiKeyInput = ref("");
const apiSecretInput = ref("");

// ── Upstox inputs ───────────────────────────────────────────────
const upstoxApiKey = ref("");
const upstoxApiSecret = ref("");

async function saveAndConnectUpstox() {
    try {
        await upstox.saveConfig(upstoxApiKey.value.trim(), upstoxApiSecret.value.trim());
        upstoxApiSecret.value = "";
        await upstox.connect();
    } catch {
        // error shown via upstox.error
    }
}

// ── Angel One inputs ────────────────────────────────────────────
const angelApiKey = ref("");
const angelClientId = ref("");
const angelPassword = ref("");
const angelTotp = ref("");

async function saveAngelConfig() {
    try {
        await angelOne.saveConfig(angelApiKey.value.trim(), angelClientId.value.trim());
    } catch {
        // error shown via angelOne.error
    }
}

async function loginAngel() {
    try {
        await angelOne.login(angelPassword.value, angelTotp.value.trim());
        angelPassword.value = "";
        angelTotp.value = "";
    } catch {
        // error shown via angelOne.error
    }
}

// ── Broker CSV shared ────────────────────────────────────────────
interface BrokerCsvRow {
    symbol: string;
    isin?: string;
    quantity: number;
    avgPrice: number;
    ltp?: number;
    exchange?: string;
}

interface ImportResult {
    imported: number;
    skipped: number;
}

async function parseBrokerCsvRust(broker: string, csvContent: string): Promise<BrokerCsvRow[]> {
    return invoke<BrokerCsvRow[]>("parse_broker_equity_csv", { broker, csvContent });
}

async function invokeBrokerCsvImport(broker: string, displayName: string, rows: BrokerCsvRow[]): Promise<ImportResult> {
    return invoke<ImportResult>("import_broker_equity_csv", {
        broker,
        displayName,
        rows: rows.map(r => ({ symbol: r.symbol, isin: r.isin, quantity: r.quantity, avgPrice: r.avgPrice, ltp: r.ltp, exchange: r.exchange })),
    });
}

// ── Zerodha XLSX parser ─────────────────────────────────────────
function parseZerodhaXlsx(buffer: ArrayBuffer): BrokerCsvRow[] {
    const wb = XLSX.read(new Uint8Array(buffer), { type: 'array' });
    const ws = wb.Sheets['Equity'];
    if (!ws) return [];
    const rows: any[] = XLSX.utils.sheet_to_json(ws, { header: 1, defval: '' });
    const headerRowIdx = rows.findIndex((row: any[]) =>
        row.some((c: any) => String(c).trim().toLowerCase() === 'symbol')
    );
    if (headerRowIdx < 0) return [];
    const rawHeaders: string[] = rows[headerRowIdx].map((h: any) =>
        String(h).trim().toLowerCase().replace(/[^a-z0-9]/g, '_').replace(/_+/g, '_').replace(/_+$/g, '')
    );
    const col = (names: string[]) => {
        const found = names.find(n => rawHeaders.includes(n));
        return found ? rawHeaders.indexOf(found) : -1;
    };
    const symbolIdx = col(['symbol', 'instrument', 'scrip', 'stock_symbol']);
    const isinIdx = col(['isin']);
    const qtyIdx = col(['quantity_available', 'qty', 'quantity', 'net_qty']);
    const avgIdx = col(['average_price', 'avg_cost', 'avg_price', 'buy_price', 'average_cost']);
    const ltpIdx = col(['previous_closing_price', 'ltp', 'last_price', 'cmp', 'close_price']);
    if (symbolIdx < 0 || qtyIdx < 0 || avgIdx < 0) return [];
    const mapped = rows.slice(headerRowIdx + 1)
        .filter((row: any[]) => row.some((c: any) => c !== ''))
        .map((row: any[]) => ({
            symbol: String(row[symbolIdx] ?? '').trim(),
            isin: isinIdx >= 0 ? (String(row[isinIdx] ?? '').trim() || undefined) : undefined,
            quantity: parseFloat(String(row[qtyIdx] ?? '')) || 0,
            avgPrice: parseFloat(String(row[avgIdx] ?? '')) || 0,
            ltp: ltpIdx >= 0 ? (parseFloat(String(row[ltpIdx] ?? '')) || undefined) : undefined,
            exchange: undefined,
        }));
    return mapped.filter(r => r.symbol && r.quantity > 0 && r.avgPrice > 0);
}


// ── Holdings CSV Dialog ────────────────────────────────────────
type AssetTab = 'equity' | 'mf' | 'fd' | 'gold' | 'crypto' | 'bond' | 'ppf_epf';

const holdingsCsvOpen   = ref(false);
const holdingsCsvStep   = ref(1);
const holdingsCsvTab    = ref<AssetTab>('equity');
const holdingsCsvBroker = ref('zerodha');
const holdingsCsvPreviewHeaders = ref<string[]>([]);
const holdingsCsvPreviewRows    = ref<Record<string, string>[]>([]);
const holdingsCsvImporting      = ref(false);
const holdingsCsvImportResult   = ref<ImportResult | null>(null);
const holdingsCsvError          = ref<string | null>(null);
let holdingsCsvRawText    = '';
let holdingsCsvEquityRows: BrokerCsvRow[] = [];

const ASSET_TAB_LABELS: Record<AssetTab, string> = {
    equity: 'Equity', mf: 'Mutual Funds', fd: 'Fixed Deposit',
    gold: 'Gold', crypto: 'Crypto', bond: 'Bonds', ppf_epf: 'PPF / EPF',
};

const CSV_TEMPLATES: Record<Exclude<AssetTab, 'equity'>, string> = {
    mf: [
        'scheme_name,isin,folio_number,units,avg_nav,current_nav,is_direct,is_growth,amc_name',
        'SBI Bluechip Direct Growth,INF200K01884,12345/678,100.5,45.23,52.10,true,true,SBI Mutual Fund',
    ].join('\n'),
    fd: [
        'bank_name,account_number,principal,interest_rate,compounding,tenure_months,start_date,maturity_date,maturity_amount,is_cumulative',
        'State Bank of India,,100000,7.25,quarterly,24,2024-01-15,2026-01-15,,true',
    ].join('\n'),
    gold: [
        'gold_type,name,weight_grams,purity,units,avg_buy_price',
        'physical,Gold Ring,10.5,22k,,4500',
        'sgb,SGB 2023-IV,,,5,5800',
        'etf,Nippon Gold ETF,,,10,5400',
    ].join('\n'),
    crypto: [
        'exchange,coin_symbol,quantity,avg_buy_price',
        'CoinDCX,BTC,0.05,3500000',
        'WazirX,ETH,1.5,150000',
    ].join('\n'),
    bond: [
        'isin,issuer_name,bond_type,face_value,quantity,purchase_price,coupon_rate,coupon_frequency,purchase_date,maturity_date,credit_rating',
        'INE001A07QO7,REC Limited,corporate,1000,10,1050,8.5,annual,2024-01-15,2029-01-15,AAA',
        ',Government of India,government,1000,5,1000,7.1,semi_annual,2024-01-15,2034-01-15,',
    ].join('\n'),
    ppf_epf: [
        'account_type,account_number,balance,interest_rate,financial_year,employer_contrib,employee_contrib',
        'PPF,PP001234567,500000,7.1,2024-25,,',
        'EPF,EMP12345678,250000,8.25,2024-25,12500,12500',
    ].join('\n'),
};

function holdingsCsvReset() {
    holdingsCsvStep.value = 1;
    holdingsCsvPreviewHeaders.value = [];
    holdingsCsvPreviewRows.value = [];
    holdingsCsvImportResult.value = null;
    holdingsCsvError.value = null;
    holdingsCsvRawText = '';
    holdingsCsvEquityRows = [];
}

function downloadHoldingsTemplate() {
    const tab = holdingsCsvTab.value as Exclude<AssetTab, 'equity'>;
    const blob = new Blob([CSV_TEMPLATES[tab]], { type: 'text/csv' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url; a.download = `template_${tab}.csv`; a.click();
    URL.revokeObjectURL(url);
    toast.add({ severity: 'success', summary: 'Template downloaded', detail: `template_${tab}.csv saved to your Downloads folder.`, life: 3000 });
}

function buildCsvPreview(content: string) {
    const lines = content.split('\n').filter(l => l.trim());
    if (lines.length < 1) return;
    const headers = lines[0].split(',').map(h => h.trim().replace(/^"|"$/g, ''));
    holdingsCsvPreviewHeaders.value = headers;
    holdingsCsvPreviewRows.value = lines.slice(1, 6).map(line => {
        const cols = line.split(',').map(c => c.trim().replace(/^"|"$/g, ''));
        const row: Record<string, string> = {};
        headers.forEach((h, i) => { row[h] = cols[i] ?? ''; });
        return row;
    });
}

function buildEquityPreview(rows: BrokerCsvRow[]) {
    holdingsCsvPreviewHeaders.value = ['Symbol', 'Qty', 'AvgPrice', 'Exchange'];
    holdingsCsvPreviewRows.value = rows.slice(0, 5).map(r => ({
        Symbol: r.symbol,
        Qty: String(r.quantity),
        AvgPrice: String(r.avgPrice),
        Exchange: r.exchange ?? '—',
    }));
    if (rows.length > 0) holdingsCsvStep.value = 2;
}

async function onHoldingsCsvFile(event: any) {
    const file: File = event.files?.[0];
    if (!file) return;
    holdingsCsvError.value = null;
    holdingsCsvPreviewHeaders.value = [];
    holdingsCsvPreviewRows.value = [];
    holdingsCsvEquityRows = [];
    holdingsCsvRawText = '';

    if (holdingsCsvTab.value === 'equity') {
        const reader = new FileReader();
        if (file.name.toLowerCase().endsWith('.xlsx')) {
            reader.onload = (e) => {
                try {
                    const parsed = parseZerodhaXlsx(e.target?.result as ArrayBuffer);
                    if (parsed.length === 0) { holdingsCsvError.value = "No valid rows found. Ensure the file has an 'Equity' sheet with Symbol, Quantity Available, and Average Price columns."; return; }
                    holdingsCsvEquityRows = parsed;
                    buildEquityPreview(parsed);
                } catch (err: any) { holdingsCsvError.value = `Parse error: ${err?.message ?? err}`; }
            };
            reader.readAsArrayBuffer(file);
        } else {
            reader.onload = async (e) => {
                try {
                    const parsed = await parseBrokerCsvRust(holdingsCsvBroker.value, e.target?.result as string);
                    if (parsed.length === 0) { holdingsCsvError.value = "No valid rows found. Check the CSV format."; return; }
                    holdingsCsvEquityRows = parsed;
                    buildEquityPreview(parsed);
                } catch (err: any) { holdingsCsvError.value = `Parse error: ${err?.message ?? err}`; }
            };
            reader.readAsText(file);
        }
    } else {
        const reader = new FileReader();
        reader.onload = (e) => {
            holdingsCsvRawText = e.target?.result as string;
            buildCsvPreview(holdingsCsvRawText);
            if (holdingsCsvPreviewRows.value.length > 0) holdingsCsvStep.value = 2;
        };
        reader.readAsText(file);
    }
}

async function importHoldingsCsv() {
    holdingsCsvImporting.value = true;
    holdingsCsvError.value = null;
    try {
        let result: ImportResult;
        if (holdingsCsvTab.value === 'equity') {
            result = await invokeBrokerCsvImport(
                holdingsCsvBroker.value,
                brokerDisplayName(holdingsCsvBroker.value),
                holdingsCsvEquityRows,
            );
        } else if (holdingsCsvTab.value === 'mf') {
            result = await invoke<ImportResult>('import_mf_csv', { csvContent: holdingsCsvRawText, accountName: 'CSV Import' });
        } else {
            result = await invoke<ImportResult>('import_generic_asset_csv', { assetType: holdingsCsvTab.value, csvContent: holdingsCsvRawText });
        }
        holdingsCsvImportResult.value = result;
        holdingsCsvStep.value = 3;
        track(`holdings_csv_import_${holdingsCsvTab.value}`, { imported: result.imported, skipped: result.skipped });
    } catch (e: any) {
        holdingsCsvError.value = String(e?.message ?? e);
    } finally {
        holdingsCsvImporting.value = false;
    }
}

function brokerDisplayName(b: string): string {
    return ({ zerodha: 'Zerodha', upstox: 'Upstox', angel_one: 'Angel One', groww: 'Groww', generic: 'Generic' } as Record<string, string>)[b] ?? b;
}

// ── Transaction CSV Import Dialog ────────────────────────────────
interface CsvPreview { headers: string[]; sampleRows: string[][]; }
interface TxnCsvMapping {
    dateCol: number | null;
    amountCol: number | null;
    categoryCol: number | null;
    descriptionCol: number | null;
    notesCol: number | null;
}
interface MappedPreviewRow {
    date: string; type: string; amount: string; category: string; description: string; notes: string;
}

const txnCsvOpen = ref(false);
const txnCsvStep = ref(1);
const txnCsvPreview = ref<CsvPreview | null>(null);
const txnCsvRawText = ref('');
const txnCsvMapping = ref<TxnCsvMapping>({
    dateCol: null, amountCol: null, categoryCol: null, descriptionCol: null, notesCol: null,
});
const txnCsvImporting = ref(false);
const txnCsvImportResult = ref<ImportResult | null>(null);
const txnCsvError = ref<string | null>(null);

function txnCsvReset() {
    txnCsvStep.value = 1;
    txnCsvPreview.value = null;
    txnCsvRawText.value = '';
    txnCsvMapping.value = { dateCol: null, amountCol: null, categoryCol: null, descriptionCol: null, notesCol: null };
    txnCsvImportResult.value = null;
    txnCsvError.value = null;
}

function guessColumnMapping(headers: string[]): TxnCsvMapping {
    const norm = headers.map(h => h.trim().toLowerCase());
    const find = (aliases: string[]) => {
        const idx = norm.findIndex(h => aliases.includes(h));
        return idx === -1 ? null : idx;
    };
    return {
        dateCol: find(['date', 'datetime', 'txn_date', 'transaction date']),
        amountCol: find(['amount', 'money', 'value']),
        categoryCol: find(['category', 'cat']),
        descriptionCol: find(['description', 'desc', 'note', 'payee', 'narration', 'people']),
        notesCol: find(['notes', 'memo', 'remarks']),
    };
}

async function onTxnCsvFile(event: any) {
    const file: File = event.files?.[0];
    if (!file) return;
    txnCsvError.value = null;
    txnCsvPreview.value = null;
    txnCsvRawText.value = '';

    const reader = new FileReader();
    reader.onload = async (e) => {
        const content = e.target?.result as string;
        txnCsvRawText.value = content;
        try {
            const preview = await invoke<CsvPreview>('preview_transaction_csv', { csvContent: content });
            if (preview.headers.length === 0) {
                txnCsvError.value = "No columns found. Check the CSV format.";
                return;
            }
            txnCsvPreview.value = preview;
            txnCsvMapping.value = guessColumnMapping(preview.headers);
            txnCsvStep.value = 2;
        } catch (err: any) {
            txnCsvError.value = `Parse error: ${err?.message ?? err}`;
        }
    };
    reader.readAsText(file);
}

const txnCsvMappedPreviewRows = computed<MappedPreviewRow[]>(() => {
    const preview = txnCsvPreview.value;
    const m = txnCsvMapping.value;
    if (!preview || m.dateCol === null || m.amountCol === null) return [];
    return preview.sampleRows.map(row => {
        const rawAmount = parseFloat((row[m.amountCol!] ?? '').replace(/,/g, ''));
        const amount = Number.isFinite(rawAmount) ? rawAmount : 0;
        return {
            date: row[m.dateCol!] ?? '',
            type: amount < 0 ? 'expense' : 'income',
            amount: formatINR(Math.abs(amount)),
            category: m.categoryCol !== null ? (row[m.categoryCol] ?? '—') : '—',
            description: m.descriptionCol !== null ? (row[m.descriptionCol] ?? '—') : '—',
            notes: m.notesCol !== null ? (row[m.notesCol] ?? '—') : '—',
        };
    });
});

async function importTxnCsv() {
    const m = txnCsvMapping.value;
    if (m.dateCol === null || m.amountCol === null) return;
    txnCsvImporting.value = true;
    txnCsvError.value = null;
    try {
        const result = await invoke<ImportResult>('import_transactions_csv', {
            csvContent: txnCsvRawText.value,
            mapping: {
                dateCol: m.dateCol,
                amountCol: m.amountCol,
                categoryCol: m.categoryCol,
                descriptionCol: m.descriptionCol,
                notesCol: m.notesCol,
            },
        });
        txnCsvImportResult.value = result;
        txnCsvStep.value = 4;
        track('txn_csv_import', { imported: result.imported, skipped: result.skipped });
    } catch (e: any) {
        txnCsvError.value = String(e?.message ?? e);
    } finally {
        txnCsvImporting.value = false;
    }
}

async function saveAndConnect() {
    try {
        await zerodha.saveConfig(apiKeyInput.value.trim(), apiSecretInput.value.trim());
        apiSecretInput.value = "";
        await zerodha.connect();
    } catch {
        // error shown via zerodha.error
    }
}

const isDevBuild = ref(false);

onMounted(async () => {
    zerodha.fetchStatus();
    upstox.fetchStatus();
    angelOne.fetchStatus();
    try { isDevBuild.value = await invoke<boolean>("is_dev_build"); } catch { /* non-critical */ }
});

// ── CAS import state ────────────────────────────────────────────
const casOpen = ref(false);
const casStep = ref(1);
const casSummaryFile = ref<File | null>(null);
const casDetailedFile = ref<File | null>(null);
const casPassword = ref("");
const casParsing = ref(false);
const casImporting = ref(false);
const casParsed = ref<CasMfHolding[]>([]);
const casError = ref<string | null>(null);
const casImportResult = ref<{ imported: number; skipped: number } | null>(null);
const casRawLines = ref<string[]>([]);

function casReset() {
    casStep.value = 1;
    casSummaryFile.value = null;
    casDetailedFile.value = null;
    casPassword.value = "";
    casParsed.value = [];
    casError.value = null;
    casImportResult.value = null;
    casRawLines.value = [];
}

function onCasSummaryFile(event: any) {
    casSummaryFile.value = event.files?.[0] ?? null;
}

function onCasDetailedFile(event: any) {
    casDetailedFile.value = event.files?.[0] ?? null;
}

async function parseCas() {
    if (!casSummaryFile.value && !casDetailedFile.value) return;
    casParsing.value = true;
    casError.value = null;
    try {
        const pw = casPassword.value.trim();
        let holdings: CasMfHolding[] = [];

        if (casSummaryFile.value && casDetailedFile.value) {
            // Both uploaded — merge for best data
            const [sumResult, detResult] = await Promise.all([
                parseCasPdf(await casSummaryFile.value.arrayBuffer(), pw),
                parseCasPdf(await casDetailedFile.value.arrayBuffer(), pw),
            ]);
            casRawLines.value = sumResult.rawLines;
            holdings = mergeCasHoldings(sumResult.holdings, detResult.holdings);
        } else {
            const file = (casSummaryFile.value ?? casDetailedFile.value)!;
            const result = await parseCasPdf(await file.arrayBuffer(), pw);
            casRawLines.value = result.rawLines;
            holdings = result.holdings;
        }

        if (holdings.length === 0) {
            casError.value =
                "No holdings found — see raw text below to check the PDF format.";
            return;
        }
        casParsed.value = holdings;
        casStep.value = 2;
    } catch (e: any) {
        const msg = String(e?.message ?? e);
        casError.value = msg.includes("PasswordException") || msg.includes("password")
            ? "Wrong password. Password = PAN (uppercase) + DOB as DDMMYYYY."
            : `Parse failed: ${msg}`;
    } finally {
        casParsing.value = false;
    }
}

async function importCas() {
    casImporting.value = true;
    casError.value = null;
    try {
        const result = await invoke<{ imported: number; skipped: number }>(
            "import_cas_mf",
            { holdings: casParsed.value },
        );
        casImportResult.value = result;
        casStep.value = 3;
        track("cas_import_completed", { imported: result.imported, skipped: result.skipped });
    } catch (e: any) {
        casError.value = String(e?.message ?? e);
    } finally {
        casImporting.value = false;
    }
}

function formatIndex(v: number | null): string {
    if (v === null) return "—";
    return v.toLocaleString("en-IN", { maximumFractionDigits: 2 });
}

function formatTime(iso: string | null): string {
    if (!iso) return "";
    try {
        return new Date(iso).toLocaleTimeString("en-IN", { hour: "2-digit", minute: "2-digit" });
    } catch {
        return "";
    }
}
</script>

<template>
    <div class="ds-view">
        <h1 class="page-title">Data Sources</h1>

        <!-- Zerodha Kite -->
        <template v-if="!isAndroid">
            <div class="section-header section-header--first">
                <h2>Zerodha Kite</h2>
                <Tag v-if="zerodha.status?.isConnected" value="Connected" severity="success" />
                <Tag v-else-if="zerodha.status?.hasConfig" value="Token Expired" severity="warn" />
            </div>

            <div class="zerodha-card">
                <!-- Waiting for browser login -->
                <div v-if="zerodha.connectLoading" class="zerodha-connecting">
                    <ProgressSpinner style="width:1.75rem;height:1.75rem" />
                    <span>Waiting for browser login… You have up to 3 minutes.</span>
                </div>

                <!-- Setup: no credentials saved yet -->
                <template v-else-if="!zerodha.status?.hasConfig">
                    <ol class="setup-instructions">
                        <li>
                            Go to <strong>kite.zerodha.com/developers</strong> and log in with
                            your Zerodha account
                        </li>
                        <li>Create a new Kite Connect app <em>(free for personal use)</em></li>
                        <li>
                            Set the redirect URL to:
                            <code class="redirect-url">http://127.0.0.1:7459</code>
                        </li>
                        <li>Copy the API Key and API Secret into the fields below</li>
                    </ol>
                    <div class="setup-form">
                        <div class="field">
                            <label for="zerodha-api-key">API Key</label>
                            <InputText
                                id="zerodha-api-key"
                                v-model="apiKeyInput"
                                placeholder="Your Kite API Key"
                                fluid
                            />
                        </div>
                        <div class="field">
                            <label for="zerodha-api-secret">API Secret</label>
                            <Password
                                inputId="zerodha-api-secret"
                                v-model="apiSecretInput"
                                :feedback="false"
                                toggleMask
                                fluid
                                placeholder="Your Kite API Secret"
                            />
                        </div>
                        <Message v-if="zerodha.error" severity="error">{{ zerodha.error }}</Message>
                        <Button
                            label="Save & Connect"
                            icon="pi pi-sign-in"
                            :disabled="!apiKeyInput.trim() || !apiSecretInput.trim()"
                            @click="saveAndConnect"
                        />
                    </div>
                </template>

                <!-- Reconnect: credentials saved but access token expired -->
                <template v-else-if="!zerodha.status?.isConnected">
                    <div class="reconnect-row">
                        <div class="reconnect-info">
                            <span class="reconnect-title">Access token expired</span>
                            <span class="reconnect-desc">
                                Zerodha access tokens expire daily at midnight IST. Reconnect
                                to continue syncing.
                            </span>
                            <span v-if="zerodha.status?.tokenDate" class="token-date text-muted">
                                Last connected: {{ zerodha.status.tokenDate }}
                            </span>
                        </div>
                        <div class="reconnect-btns">
                            <Button
                                label="Reconnect"
                                icon="pi pi-sign-in"
                                @click="zerodha.connect()"
                            />
                            <Button
                                label="Remove credentials"
                                text
                                size="small"
                                @click="zerodha.disconnect()"
                            />
                        </div>
                    </div>
                    <Message v-if="zerodha.error" severity="error" class="mt-msg">
                        {{ zerodha.error }}
                    </Message>
                </template>

                <!-- Connected: ready to sync -->
                <template v-else>
                    <div class="sync-row">
                        <div class="sync-info">
                            <span class="sync-title">Zerodha connected</span>
                            <span class="sync-desc text-muted">
                                Sync to import your latest holdings into the Portfolio tab.
                            </span>
                            <span v-if="zerodha.syncResult" class="sync-result text-muted">
                                Last sync: {{ zerodha.syncResult.synced }} holding{{
                                    zerodha.syncResult.synced !== 1 ? "s" : ""
                                }} imported
                            </span>
                        </div>
                        <div class="sync-btns">
                            <Button
                                label="Sync Holdings"
                                icon="pi pi-refresh"
                                :loading="zerodha.syncLoading"
                                @click="zerodha.syncHoldings()"
                            />
                            <Button
                                label="Disconnect"
                                text
                                size="small"
                                @click="zerodha.disconnect()"
                            />
                        </div>
                    </div>
                    <Message v-if="zerodha.error" severity="error" class="mt-msg">
                        {{ zerodha.error }}
                    </Message>
                    <ul v-if="zerodha.syncResult?.errors?.length" class="sync-errors">
                        <li v-for="err in zerodha.syncResult.errors" :key="err">{{ err }}</li>
                    </ul>
                </template>

                <p class="csv-redirect-hint">
                    To import from CSV, use
                    <button class="link-btn" @click="holdingsCsvOpen = true; holdingsCsvTab = 'equity'; holdingsCsvBroker = 'zerodha'">Holdings CSV Import</button>
                    in the Import section above.
                </p>
            </div>
        </template>
        <template v-else>
            <div class="section-header section-header--first">
                <h2>Zerodha Kite</h2>
                <Tag value="Desktop Only" severity="secondary" />
            </div>
            <div class="zerodha-card">
                <div class="desktop-only-notice">
                    <i class="pi pi-desktop desktop-only-icon" />
                    <div class="desktop-only-text">
                        <span class="reconnect-title">Connect on Desktop</span>
                        <span class="reconnect-desc">
                            Zerodha OAuth uses a browser redirect to <code>localhost</code>, which
                            is not available on Android. Connect on your desktop app, then use
                            <strong>Settings → Export Sync Backup</strong> to transfer holdings
                            to this device.
                        </span>
                    </div>
                </div>
                <p class="csv-redirect-hint">
                    To import from CSV, use
                    <button class="link-btn" @click="holdingsCsvOpen = true; holdingsCsvTab = 'equity'; holdingsCsvBroker = 'zerodha'">Holdings CSV Import</button>
                    in the Import section above.
                </p>
            </div>
        </template>

        <!-- ═══════════════════════════════════════════ Upstox ══ -->
        <template v-if="!isAndroid">
            <div class="section-header">
                <h2>Upstox</h2>
                <Tag v-if="upstox.status?.isConnected" value="Connected" severity="success" />
                <Tag v-else-if="upstox.status?.hasConfig" value="Token Expired" severity="warn" />
            </div>

            <div class="zerodha-card">
                <!-- Waiting for browser login -->
                <div v-if="upstox.connectLoading" class="zerodha-connecting">
                    <ProgressSpinner style="width:1.75rem;height:1.75rem" />
                    <span>Waiting for browser login… You have up to 3 minutes.</span>
                </div>

                <!-- Setup: no credentials saved yet -->
                <template v-else-if="!upstox.status?.hasConfig">
                    <ol class="setup-instructions">
                        <li>
                            Go to <strong>developer.upstox.com</strong> → My Apps → Create App
                        </li>
                        <li>
                            Set the redirect URL to:
                            <code class="redirect-url">http://127.0.0.1:7460</code>
                        </li>
                        <li>Copy the API Key and API Secret into the fields below</li>
                    </ol>
                    <div class="setup-form">
                        <div class="field">
                            <label for="upstox-api-key">API Key</label>
                            <InputText id="upstox-api-key" v-model="upstoxApiKey" placeholder="Your Upstox API Key" fluid />
                        </div>
                        <div class="field">
                            <label for="upstox-api-secret">API Secret</label>
                            <Password
                                inputId="upstox-api-secret"
                                v-model="upstoxApiSecret"
                                :feedback="false"
                                toggleMask
                                fluid
                                placeholder="Your Upstox API Secret"
                            />
                        </div>
                        <Message v-if="upstox.error" severity="error">{{ upstox.error }}</Message>
                        <Button
                            label="Save & Connect"
                            icon="pi pi-sign-in"
                            :disabled="!upstoxApiKey.trim() || !upstoxApiSecret.trim()"
                            @click="saveAndConnectUpstox"
                        />
                    </div>
                </template>

                <!-- Reconnect: credentials saved but token expired -->
                <template v-else-if="!upstox.status?.isConnected">
                    <div class="reconnect-row">
                        <div class="reconnect-info">
                            <span class="reconnect-title">Access token expired</span>
                            <span class="reconnect-desc">
                                Upstox access tokens expire daily. Reconnect to continue syncing.
                            </span>
                            <span v-if="upstox.status?.tokenDate" class="token-date text-muted">
                                Last connected: {{ upstox.status.tokenDate }}
                            </span>
                        </div>
                        <div class="reconnect-btns">
                            <Button label="Reconnect" icon="pi pi-sign-in" @click="upstox.connect()" />
                            <Button label="Remove credentials" text size="small" @click="upstox.disconnect()" />
                        </div>
                    </div>
                    <Message v-if="upstox.error" severity="error" class="mt-msg">{{ upstox.error }}</Message>
                </template>

                <!-- Connected -->
                <template v-else>
                    <div class="sync-row">
                        <div class="sync-info">
                            <span class="sync-title">Upstox connected</span>
                            <span class="sync-desc text-muted">
                                Sync to import your latest holdings into the Portfolio tab.
                            </span>
                            <span v-if="upstox.syncResult" class="sync-result text-muted">
                                Last sync: {{ upstox.syncResult.synced }} holding{{
                                    upstox.syncResult.synced !== 1 ? "s" : ""
                                }} imported
                            </span>
                        </div>
                        <div class="sync-btns">
                            <Button
                                label="Sync Holdings"
                                icon="pi pi-refresh"
                                :loading="upstox.syncLoading"
                                @click="upstox.syncHoldings()"
                            />
                            <Button label="Disconnect" text size="small" @click="upstox.disconnect()" />
                        </div>
                    </div>
                    <Message v-if="upstox.error" severity="error" class="mt-msg">{{ upstox.error }}</Message>
                </template>

                <p class="csv-redirect-hint">
                    To import from CSV, use
                    <button class="link-btn" @click="holdingsCsvOpen = true; holdingsCsvTab = 'equity'; holdingsCsvBroker = 'upstox'">Holdings CSV Import</button>
                    in the Import section above.
                </p>
            </div>
        </template>
        <template v-else>
            <div class="section-header">
                <h2>Upstox</h2>
                <Tag value="Desktop Only" severity="secondary" />
            </div>
            <div class="zerodha-card">
                <div class="desktop-only-notice">
                    <i class="pi pi-desktop desktop-only-icon" />
                    <div class="desktop-only-text">
                        <span class="reconnect-title">Connect on Desktop</span>
                        <span class="reconnect-desc">
                            Upstox OAuth uses a browser redirect to <code>localhost</code>, which
                            is not available on Android. Connect on your desktop app, then sync
                            holdings via <strong>Settings → Export Sync Backup</strong>.
                        </span>
                    </div>
                </div>
                <p class="csv-redirect-hint">
                    To import from CSV, use
                    <button class="link-btn" @click="holdingsCsvOpen = true; holdingsCsvTab = 'equity'; holdingsCsvBroker = 'upstox'">Holdings CSV Import</button>
                    in the Import section above.
                </p>
            </div>
        </template>

        <!-- ══════════════════════════════════════════ Angel One ══ -->
        <div class="section-header">
            <h2>Angel One</h2>
            <Tag v-if="angelOne.status?.isConnected" value="Connected" severity="success" />
            <Tag v-else-if="angelOne.status?.hasConfig" value="Token Expired" severity="warn" />
        </div>

        <div class="zerodha-card">
            <!-- Setup: no API key / client ID saved -->
            <template v-if="!angelOne.status?.hasConfig">
                <p class="reconnect-desc" style="margin:0 0 1rem">
                    Angel One uses SmartAPI — no browser redirect. Enter your API Key and
                    Client Code (your Angel One login ID), then login with password + TOTP
                    each day.
                </p>
                <div class="setup-form">
                    <div class="field">
                        <label for="angel-api-key">API Key</label>
                        <InputText id="angel-api-key" v-model="angelApiKey" placeholder="SmartAPI API Key" fluid />
                    </div>
                    <div class="field">
                        <label for="angel-client-id">Client Code</label>
                        <InputText id="angel-client-id" v-model="angelClientId" placeholder="Your Angel One Client Code" fluid />
                    </div>
                    <Message v-if="angelOne.error" severity="error">{{ angelOne.error }}</Message>
                    <Button
                        label="Save Config"
                        icon="pi pi-save"
                        :disabled="!angelApiKey.trim() || !angelClientId.trim()"
                        @click="saveAngelConfig"
                    />
                </div>
            </template>

            <!-- Has config but no JWT / token expired — show login form -->
            <template v-else-if="!angelOne.status?.isConnected">
                <div class="reconnect-row">
                    <div class="reconnect-info">
                        <span class="reconnect-title">Login required</span>
                        <span class="reconnect-desc">
                            Angel One JWT tokens expire daily. Enter your password and TOTP to reconnect.
                        </span>
                        <span v-if="angelOne.status?.tokenDate" class="token-date text-muted">
                            Last connected: {{ angelOne.status.tokenDate }}
                        </span>
                    </div>
                </div>
                <div class="setup-form" style="margin-top:1rem">
                    <div class="field">
                        <label>Password</label>
                        <Password
                            v-model="angelPassword"
                            :feedback="false"
                            toggleMask
                            fluid
                            placeholder="Angel One login password"
                        />
                    </div>
                    <div class="field">
                        <label>
                            TOTP
                            <span class="text-muted" style="font-weight:400"> — 6-digit OTP from your authenticator app</span>
                        </label>
                        <InputText v-model="angelTotp" placeholder="123456" maxlength="6" fluid />
                    </div>
                    <Message v-if="angelOne.error" severity="error">{{ angelOne.error }}</Message>
                    <div style="display:flex;gap:0.5rem;flex-wrap:wrap">
                        <Button
                            label="Login"
                            icon="pi pi-sign-in"
                            :loading="angelOne.loginLoading"
                            :disabled="!angelPassword || !angelTotp.trim()"
                            @click="loginAngel"
                        />
                        <Button label="Remove credentials" text size="small" @click="angelOne.disconnect()" />
                    </div>
                </div>
            </template>

            <!-- Connected -->
            <template v-else>
                <div class="sync-row">
                    <div class="sync-info">
                        <span class="sync-title">Angel One connected</span>
                        <span class="sync-desc text-muted">
                            Sync to import your latest holdings. JWT expires daily — re-login tomorrow.
                        </span>
                        <span v-if="angelOne.syncResult" class="sync-result text-muted">
                            Last sync: {{ angelOne.syncResult.synced }} holding{{
                                angelOne.syncResult.synced !== 1 ? "s" : ""
                            }} imported
                        </span>
                    </div>
                    <div class="sync-btns">
                        <Button
                            label="Sync Holdings"
                            icon="pi pi-refresh"
                            :loading="angelOne.syncLoading"
                            @click="angelOne.syncHoldings()"
                        />
                        <Button label="Disconnect" text size="small" @click="angelOne.disconnect()" />
                    </div>
                </div>
                <Message v-if="angelOne.error" severity="error" class="mt-msg">{{ angelOne.error }}</Message>
            </template>

            <p class="csv-redirect-hint">
                To import from CSV, use
                <button class="link-btn" @click="holdingsCsvOpen = true; holdingsCsvTab = 'equity'; holdingsCsvBroker = 'angel_one'">Holdings CSV Import</button>
                in the Import section above.
            </p>
        </div>

        <!-- Market Pulse (dev only) -->
        <template v-if="isDevBuild">
        <div class="section-header">
            <h2>Market Pulse</h2>
            <Button
                icon="pi pi-refresh"
                label="Fetch"
                size="small"
                text
                :loading="store.indicesLoading"
                @click="store.fetchIndices()"
                v-tooltip="'Fetch live market data'"
            />
        </div>

        <div class="indices-row">
            <div class="index-card">
                <span class="index-label">Nifty 50</span>
                <span class="index-value">{{ formatIndex(store.indices?.nifty50 ?? null) }}</span>
            </div>
            <div class="index-card">
                <span class="index-label">Sensex</span>
                <span class="index-value">{{ formatIndex(store.indices?.sensex ?? null) }}</span>
            </div>
            <div class="index-card">
                <span class="index-label">USD / INR</span>
                <span class="index-value">
                    {{ store.indices?.usdInr ? "₹" + store.indices.usdInr.toFixed(2) : "—" }}
                </span>
            </div>
        </div>
        <p v-if="store.indices?.lastUpdated" class="indices-updated">
            Last fetched at {{ formatTime(store.indices.lastUpdated) }}
        </p>
        <p v-else class="indices-updated">Click Fetch to load live indices.</p>
        </template>

        <!-- Price Refresh -->
        <div class="section-header">
            <h2>Price Refresh</h2>
        </div>

        <div class="refresh-grid">
            <!-- Equity prices (dev only) -->
            <div v-if="isDevBuild" class="refresh-card">
                <div class="refresh-card-header">
                    <div>
                        <span class="refresh-title">Equity Holdings</span>
                        <p class="refresh-desc">
                            Updates <code>current_price</code> for all NSE / BSE holdings
                            via market data feed (<code>{SYMBOL}.NS</code> / <code>.BO</code>).
                        </p>
                    </div>
                    <Button
                        icon="pi pi-refresh"
                        label="Refresh Prices"
                        size="small"
                        :loading="store.equityLoading"
                        @click="store.refreshEquity()"
                    />
                </div>
                <div v-if="store.equityResult" class="refresh-result" aria-live="polite" aria-atomic="true">
                    <Tag :value="`✓ ${store.equityResult.updated} updated`" />
                    <Tag
                        v-if="store.equityResult.failed > 0"
                        :value="`✗ ${store.equityResult.failed} failed`"
                    />
                    <ul v-if="store.equityResult.errors.length" class="error-list">
                        <li v-for="err in store.equityResult.errors" :key="err">{{ err }}</li>
                    </ul>
                </div>
            </div>

            <!-- MF -->
            <div class="refresh-card">
                <div class="refresh-card-header">
                    <div>
                        <span class="refresh-title">Mutual Fund NAV</span>
                        <p class="refresh-desc">
                            Updates <code>current_nav</code> for all MF holdings
                            via <code>mfapi.in</code> (AMFI official data, free).
                        </p>
                    </div>
                    <Button
                        icon="pi pi-refresh"
                        label="Refresh NAVs"
                        size="small"
                        :loading="store.mfLoading"
                        @click="store.refreshMfNav()"
                    />
                </div>
                <div v-if="store.mfResult" class="refresh-result" aria-live="polite" aria-atomic="true">
                    <Tag :value="`✓ ${store.mfResult.updated} updated`" />
                    <Tag
                        v-if="store.mfResult.failed > 0"
                        :value="`✗ ${store.mfResult.failed} failed`"
                    />
                    <ul v-if="store.mfResult.errors.length" class="error-list">
                        <li v-for="err in store.mfResult.errors" :key="err">{{ err }}</li>
                    </ul>
                </div>
            </div>
        </div>

        <!-- Import -->
        <div class="section-header">
            <h2>Import</h2>
        </div>

        <div class="import-grid">
            <div class="import-card import-card--active">
                <i class="pi pi-file-excel import-icon" />
                <span class="import-title">CSV Import</span>
                <span class="import-desc">
                    Import transactions from any CSV with a column-mapping wizard.
                </span>
                <Button
                    label="Import CSV"
                    icon="pi pi-upload"
                    size="small"
                    @click="txnCsvOpen = true"
                />
            </div>
            <div class="import-card">
                <i class="pi pi-file-pdf import-icon" />
                <span class="import-title">Bank Statement (PDF)</span>
                <span class="import-desc">
                    Auto-parse statements from HDFC, SBI, ICICI, and Axis Bank.
                </span>
                <Tag value="Coming Soon" />
            </div>
            <!-- MF Central CAS — active -->
            <div class="import-card import-card--active">
                <i class="pi pi-file import-icon" />
                <span class="import-title">MF Central CAS</span>
                <span class="import-desc">
                    Import your consolidated account statement from MF Central.
                </span>
                <Button
                    label="Import CAS"
                    icon="pi pi-upload"
                    size="small"
                    @click="casOpen = true"
                />
            </div>
            <!-- Holdings CSV Import — active -->
            <div class="import-card import-card--active">
                <i class="pi pi-table import-icon" />
                <span class="import-title">Holdings CSV Import</span>
                <span class="import-desc">
                    Import equity, MF, FD, gold, crypto, bonds, or PPF/EPF from a CSV using our templates.
                </span>
                <Button
                    label="Import CSV"
                    icon="pi pi-upload"
                    size="small"
                    @click="holdingsCsvOpen = true"
                />
            </div>
        </div>

        <!-- MF Central CAS import dialog -->
        <Dialog
            v-model:visible="casOpen"
            header="Import MF Central CAS"
            :modal="true"
            :style="{ width: '680px', maxWidth: '95vw' }"
            @hide="casReset"
        >
            <!-- Step 1: file + password -->
            <template v-if="casStep === 1">
                <p class="cas-hint">
                    Upload one or both CAS files from
                    <strong>MF Central</strong>. Uploading both gives the best result —
                    ISINs from Detailed + cost basis from Summary.
                </p>
                <div class="cas-form">
                    <div class="cas-file-row">
                        <div class="field cas-file-slot">
                            <label>
                                Summary CAS
                                <Tag value="Avg NAV / P&amp;L" severity="info" size="small" class="ml-1" />
                            </label>
                            <FileUpload
                                mode="basic"
                                accept=".pdf"
                                :maxFileSize="20000000"
                                chooseLabel="Choose PDF"
                                customUpload
                                auto
                                @uploader="onCasSummaryFile"
                            />
                            <span v-if="casSummaryFile" class="cas-filename">{{ casSummaryFile.name }}</span>
                        </div>
                        <div class="field cas-file-slot">
                            <label>
                                Detailed CAS
                                <Tag value="ISINs" severity="success" size="small" class="ml-1" />
                            </label>
                            <FileUpload
                                mode="basic"
                                accept=".pdf"
                                :maxFileSize="20000000"
                                chooseLabel="Choose PDF"
                                customUpload
                                auto
                                @uploader="onCasDetailedFile"
                            />
                            <span v-if="casDetailedFile" class="cas-filename">{{ casDetailedFile.name }}</span>
                        </div>
                    </div>
                    <div class="field">
                        <label>
                            Password
                            <span class="cas-hint-inline">(PAN + DOB — same for both files)</span>
                        </label>
                        <Password
                            v-model="casPassword"
                            :feedback="false"
                            toggleMask
                            fluid
                            placeholder="e.g. ABCDE1234F01011990"
                        />
                    </div>
                    <Message v-if="casError" severity="error">{{ casError }}</Message>
                    <Button
                        label="Parse PDF"
                        icon="pi pi-search"
                        :loading="casParsing"
                        :disabled="!casSummaryFile && !casDetailedFile"
                        @click="parseCas"
                    />
                    <!-- Raw text debug panel — shown only when parsing produced no holdings -->
                    <div v-if="casRawLines.length && !casParsed.length" class="cas-debug">
                        <p class="cas-debug-label">
                            Raw extracted text (first 300 lines) — share this to diagnose
                            parser issues:
                        </p>
                        <Textarea
                            :modelValue="casRawLines.join('\n')"
                            :rows="12"
                            readonly
                            fluid
                            class="cas-debug-text"
                        />
                    </div>
                </div>
            </template>

            <!-- Step 2: preview + confirm -->
            <template v-else-if="casStep === 2">
                <p class="cas-hint">
                    Found <strong>{{ casParsed.length }}</strong> holding{{
                        casParsed.length !== 1 ? "s" : ""
                    }}. Review below then click Import.
                </p>
                <DataTable
                    :value="casParsed"
                    size="small"
                    scrollable
                    scrollHeight="300px"
                    class="cas-table"
                >
                    <Column field="schemeName" header="Scheme" style="min-width:220px" />
                    <Column field="folioNumber" header="Folio" style="width:120px" />
                    <Column header="Units" style="width:100px">
                        <template #body="{ data }">
                            {{ data.units.toLocaleString("en-IN", { maximumFractionDigits: 3 }) }}
                        </template>
                    </Column>
                    <Column header="Avg NAV" style="width:90px">
                        <template #body="{ data }">
                            {{ data.avgNav > 0 ? data.avgNav.toLocaleString("en-IN", { maximumFractionDigits: 4 }) : "—" }}
                        </template>
                    </Column>
                    <Column header="NAV" style="width:90px">
                        <template #body="{ data }">
                            {{ data.nav.toLocaleString("en-IN", { maximumFractionDigits: 4 }) }}
                        </template>
                    </Column>
                    <Column header="Type" style="width:80px">
                        <template #body="{ data }">
                            <Tag
                                :value="data.isDirect ? 'Direct' : 'Regular'"
                                :severity="data.isDirect ? 'success' : 'secondary'"
                                size="small"
                            />
                        </template>
                    </Column>
                </DataTable>
                <Message v-if="casError" severity="error" class="mt-msg">{{ casError }}</Message>
                <div class="cas-confirm-btns">
                    <Button
                        label="Back"
                        icon="pi pi-arrow-left"
                        text
                        @click="casStep = 1"
                    />
                    <Button
                        :label="`Import ${casParsed.length} Holdings`"
                        icon="pi pi-check"
                        :loading="casImporting"
                        @click="importCas"
                    />
                </div>
            </template>

            <!-- Step 3: done -->
            <template v-else-if="casStep === 3">
                <div class="cas-done">
                    <i class="pi pi-check-circle cas-done-icon" />
                    <p>
                        <strong>{{ casImportResult?.imported }}</strong> holdings imported
                        <span v-if="casImportResult?.skipped">
                            ({{ casImportResult.skipped }} skipped — zero units)
                        </span>
                    </p>
                    <p class="text-muted">
                        MF holdings are now in the Portfolio tab. Click Refresh NAVs in Price
                        Refresh to update prices.
                    </p>
                    <Button label="Done" @click="casOpen = false" />
                </div>
            </template>
        </Dialog>

        <!-- Holdings CSV Import dialog -->
        <Dialog
            v-model:visible="holdingsCsvOpen"
            header="Holdings CSV Import"
            :modal="true"
            :style="{ width: '720px', maxWidth: '95vw' }"
            @hide="holdingsCsvReset"
        >
            <!-- Step 1: choose asset type + upload -->
            <template v-if="holdingsCsvStep === 1">
                <p class="cas-hint">
                    Choose an asset type, then upload your CSV. For non-equity types, download
                    the template, fill in your holdings, then upload.
                </p>

                <!-- Asset type pill tabs -->
                <div class="hcsv-tabs">
                    <button
                        v-for="(label, key) in ASSET_TAB_LABELS"
                        :key="key"
                        class="hcsv-tab"
                        :class="{ 'hcsv-tab--active': holdingsCsvTab === key }"
                        @click="holdingsCsvTab = (key as AssetTab); holdingsCsvReset(); holdingsCsvStep = 1"
                    >{{ label }}</button>
                </div>

                <div class="hcsv-body">
                    <!-- Equity tab -->
                    <template v-if="holdingsCsvTab === 'equity'">
                        <div class="field" style="margin-bottom: 0.75rem">
                            <label style="font-size: 0.83rem; font-weight: 600; display: block; margin-bottom: 0.35rem">Broker</label>
                            <Select
                                v-model="holdingsCsvBroker"
                                :options="[
                                    { label: 'Zerodha', value: 'zerodha' },
                                    { label: 'Upstox', value: 'upstox' },
                                    { label: 'Angel One', value: 'angel_one' },
                                    { label: 'Groww', value: 'groww' },
                                    { label: 'Generic', value: 'generic' },
                                ]"
                                optionLabel="label"
                                optionValue="value"
                                style="width: 220px"
                            />
                        </div>
                        <p class="reconnect-desc" style="margin: 0 0 0.75rem">
                            Zerodha: download XLSX or CSV from console.zerodha.com → Portfolio → Holdings → Download.
                            Other brokers: use the Holdings export from their portal.
                        </p>
                        <FileUpload
                            mode="basic"
                            accept=".xlsx,.csv"
                            :maxFileSize="5000000"
                            chooseLabel="Choose XLSX / CSV"
                            customUpload
                            auto
                            @uploader="onHoldingsCsvFile"
                        />
                    </template>

                    <!-- Non-equity tabs -->
                    <template v-else>
                        <p class="reconnect-desc" style="margin: 0 0 0.75rem">
                            Download the template, fill in your holdings, then upload.
                        </p>
                        <div class="hcsv-upload-row">
                            <Button
                                label="Download Template"
                                icon="pi pi-download"
                                size="small"
                                outlined
                                @click="downloadHoldingsTemplate"
                            />
                            <FileUpload
                                mode="basic"
                                customUpload
                                auto
                                accept=".csv"
                                chooseLabel="Choose CSV"
                                :chooseIcon="'pi pi-upload'"
                                @uploader="onHoldingsCsvFile"
                            />
                        </div>
                    </template>

                    <Message v-if="holdingsCsvError" severity="error" class="mt-msg">{{ holdingsCsvError }}</Message>
                </div>
            </template>

            <!-- Step 2: preview + import -->
            <template v-else-if="holdingsCsvStep === 2">
                <p class="cas-hint">
                    Found
                    <strong>{{ holdingsCsvTab === 'equity' ? holdingsCsvEquityRows.length : holdingsCsvPreviewRows.length }}</strong>
                    row{{ (holdingsCsvTab === 'equity' ? holdingsCsvEquityRows.length : holdingsCsvPreviewRows.length) !== 1 ? 's' : '' }} ready to import
                    (preview shows first {{ holdingsCsvPreviewRows.length }}).
                </p>
                <DataTable
                    :value="holdingsCsvPreviewRows"
                    size="small"
                    scrollable
                    scrollHeight="280px"
                    class="cas-table"
                >
                    <Column
                        v-for="h in holdingsCsvPreviewHeaders"
                        :key="h"
                        :field="h"
                        :header="h"
                        style="min-width: 110px"
                    />
                </DataTable>
                <Message v-if="holdingsCsvError" severity="error" class="mt-msg">{{ holdingsCsvError }}</Message>
                <div class="cas-confirm-btns">
                    <Button label="Back" icon="pi pi-arrow-left" text @click="holdingsCsvStep = 1" />
                    <Button
                        :label="`Import ${ holdingsCsvTab === 'equity' ? holdingsCsvEquityRows.length : holdingsCsvPreviewRows.length } Holdings`"
                        icon="pi pi-check"
                        :loading="holdingsCsvImporting"
                        @click="importHoldingsCsv"
                    />
                </div>
            </template>

            <!-- Step 3: done -->
            <template v-else-if="holdingsCsvStep === 3">
                <div class="cas-done">
                    <i class="pi pi-check-circle cas-done-icon" />
                    <p>
                        <strong>{{ holdingsCsvImportResult?.imported }}</strong> holdings imported
                        <span v-if="holdingsCsvImportResult?.skipped">
                            ({{ holdingsCsvImportResult.skipped }} skipped)
                        </span>
                    </p>
                    <p class="text-muted">
                        <template v-if="holdingsCsvTab === 'equity'">Prices update on next Refresh Prices.</template>
                        <template v-else-if="holdingsCsvTab === 'mf'">Click Refresh NAVs in Price Refresh to update MF prices.</template>
                        <template v-else>Holdings are now visible in the Portfolio tab.</template>
                    </p>
                    <Button label="Done" @click="holdingsCsvOpen = false" />
                </div>
            </template>
        </Dialog>

        <!-- Transaction CSV Import dialog -->
        <Dialog
            v-model:visible="txnCsvOpen"
            header="CSV Import"
            :modal="true"
            :style="{ width: '720px', maxWidth: '95vw' }"
            @hide="txnCsvReset"
        >
            <!-- Step 1: upload -->
            <template v-if="txnCsvStep === 1">
                <p class="cas-hint">
                    Works with exports from MoneyWallet and most expense trackers — upload any
                    transactions CSV and you'll map its columns to Date, Amount, Category, etc. next.
                </p>
                <FileUpload
                    mode="basic"
                    accept=".csv"
                    customUpload
                    auto
                    chooseLabel="Choose CSV"
                    @uploader="onTxnCsvFile"
                />
                <Message v-if="txnCsvError" severity="error" class="mt-msg">{{ txnCsvError }}</Message>
            </template>

            <!-- Step 2: column mapping -->
            <template v-else-if="txnCsvStep === 2 && txnCsvPreview">
                <p class="cas-hint">Map the CSV columns to transaction fields.</p>
                <div class="cas-form">
                <div class="field-row">
                    <div class="field">
                        <label>Date *</label>
                        <Select
                            v-model="txnCsvMapping.dateCol"
                            :options="txnCsvPreview.headers.map((h, i) => ({ label: h, value: i }))"
                            optionLabel="label"
                            optionValue="value"
                            placeholder="Select column…"
                            class="w-full"
                        />
                    </div>
                    <div class="field">
                        <label>Amount *</label>
                        <Select
                            v-model="txnCsvMapping.amountCol"
                            :options="txnCsvPreview.headers.map((h, i) => ({ label: h, value: i }))"
                            optionLabel="label"
                            optionValue="value"
                            placeholder="Select column…"
                            class="w-full"
                        />
                    </div>
                </div>
                <div class="field-row">
                    <div class="field">
                        <label>Category</label>
                        <Select
                            v-model="txnCsvMapping.categoryCol"
                            :options="[{ label: '— None —', value: null }, ...txnCsvPreview.headers.map((h, i) => ({ label: h, value: i }))]"
                            optionLabel="label"
                            optionValue="value"
                            placeholder="— None —"
                            class="w-full"
                        />
                    </div>
                    <div class="field">
                        <label>Description</label>
                        <Select
                            v-model="txnCsvMapping.descriptionCol"
                            :options="[{ label: '— None —', value: null }, ...txnCsvPreview.headers.map((h, i) => ({ label: h, value: i }))]"
                            optionLabel="label"
                            optionValue="value"
                            placeholder="— None —"
                            class="w-full"
                        />
                    </div>
                    <div class="field">
                        <label>Notes</label>
                        <Select
                            v-model="txnCsvMapping.notesCol"
                            :options="[{ label: '— None —', value: null }, ...txnCsvPreview.headers.map((h, i) => ({ label: h, value: i }))]"
                            optionLabel="label"
                            optionValue="value"
                            placeholder="— None —"
                            class="w-full"
                        />
                    </div>
                </div>
                </div>
                <Message v-if="txnCsvError" severity="error" class="mt-msg">{{ txnCsvError }}</Message>
                <div class="cas-confirm-btns">
                    <Button label="Back" icon="pi pi-arrow-left" text @click="txnCsvStep = 1" />
                    <Button
                        label="Next"
                        icon="pi pi-arrow-right"
                        iconPos="right"
                        :disabled="txnCsvMapping.dateCol === null || txnCsvMapping.amountCol === null"
                        @click="txnCsvStep = 3"
                    />
                </div>
            </template>

            <!-- Step 3: preview -->
            <template v-else-if="txnCsvStep === 3">
                <p class="cas-hint">
                    Preview of the first {{ txnCsvMappedPreviewRows.length }} rows. Amounts are shown
                    positive with the inferred type — negative source amounts import as expenses.
                </p>
                <DataTable :value="txnCsvMappedPreviewRows" size="small" scrollable scrollHeight="280px" class="cas-table">
                    <Column field="date" header="Date" style="min-width: 140px" />
                    <Column field="type" header="Type" style="min-width: 90px" />
                    <Column field="amount" header="Amount" style="min-width: 110px" />
                    <Column field="category" header="Category" style="min-width: 110px" />
                    <Column field="description" header="Description" style="min-width: 140px" />
                    <Column field="notes" header="Notes" style="min-width: 110px" />
                </DataTable>
                <Message v-if="txnCsvError" severity="error" class="mt-msg">{{ txnCsvError }}</Message>
                <div class="cas-confirm-btns">
                    <Button label="Back" icon="pi pi-arrow-left" text @click="txnCsvStep = 2" />
                    <Button label="Import" icon="pi pi-check" :loading="txnCsvImporting" @click="importTxnCsv" />
                </div>
            </template>

            <!-- Step 4: done -->
            <template v-else-if="txnCsvStep === 4">
                <div class="cas-done">
                    <i class="pi pi-check-circle cas-done-icon" />
                    <p>
                        <strong>{{ txnCsvImportResult?.imported }}</strong> transactions imported
                        <span v-if="txnCsvImportResult?.skipped">
                            ({{ txnCsvImportResult.skipped }} skipped — duplicates or unparseable rows)
                        </span>
                    </p>
                    <p class="text-muted">Imported transactions are now visible in the Transactions view.</p>
                    <Button label="Done" @click="txnCsvOpen = false" />
                </div>
            </template>
        </Dialog>

    </div>
</template>

<style scoped>
.ds-view { max-width: 1000px; }
.page-title { font-size: 1.5rem; font-weight: 700; margin: 0 0 1.5rem; }

.section-header {
    display: flex; justify-content: space-between; align-items: center;
    margin: 1.75rem 0 0.75rem;
}
.section-header:first-of-type { margin-top: 0; }
.section-header h2 { margin: 0; font-size: 1.1rem; font-weight: 600; }

/* Market indices */
.indices-row { display: flex; gap: 1rem; flex-wrap: wrap; }
.index-card {
    flex: 1; min-width: 140px; border-radius: 10px;
    padding: 1rem 1.25rem;
    display: flex; flex-direction: column; gap: 0.3rem;
    background: var(--p-content-background);
    border: 1px solid var(--p-content-border-color);
}
.index-label { font-size: 0.78rem; font-weight: 500; color: var(--p-text-muted-color); }
.index-value { font-size: 1.3rem; font-weight: 700; font-variant-numeric: tabular-nums; }
.indices-updated { font-size: 0.78rem; margin: 0.5rem 0 0; color: var(--p-text-muted-color); }

/* Price refresh cards */
.refresh-grid { display: flex; flex-direction: column; gap: 1rem; }
.refresh-card { border-radius: 12px; padding: 1.25rem 1.5rem; background: var(--p-content-background); border: 1px solid var(--p-content-border-color); }
.refresh-card-header { display: flex; justify-content: space-between; align-items: flex-start; gap: 1rem; }
.refresh-title { font-size: 1rem; font-weight: 600; display: block; margin-bottom: 0.35rem; }
.refresh-desc { font-size: 0.83rem; margin: 0; color: var(--p-text-muted-color); }
.refresh-result { display: flex; flex-wrap: wrap; align-items: center; gap: 0.5rem; margin-top: 0.75rem; }
.error-list { margin: 0.25rem 0 0; padding-left: 1.25rem; font-size: 0.8rem; width: 100%; }
.error-list li { margin-bottom: 0.2rem; }

/* Import cards */
.import-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 1rem; }
.import-card {
    border-radius: 12px; padding: 1.25rem;
    display: flex; flex-direction: column; align-items: flex-start; gap: 0.5rem;
    background: var(--p-content-background);
    border: 1px dashed var(--p-content-border-color);
    opacity: 0.75;
    cursor: not-allowed;
}
.import-icon { font-size: 1.5rem; }
.import-title { font-size: 0.95rem; font-weight: 600; }
.import-desc { font-size: 0.82rem; line-height: 1.45; }

/* Active import card */
.import-card--active {
    opacity: 1;
    cursor: default;
    border-style: solid;
}

/* CAS dialog */
.cas-hint {
    font-size: 0.85rem;
    color: var(--p-text-muted-color);
    margin: 0 0 1.25rem;
    line-height: 1.6;
}
.cas-hint code {
    font-size: 0.82rem;
    padding: 0.1rem 0.35rem;
    border-radius: 4px;
    background: var(--p-content-hover-background);
    font-family: monospace;
}
.cas-form {
    display: flex; flex-direction: column; gap: 0.9rem;
}
.cas-form .field {
    display: flex; flex-direction: column; gap: 0.35rem;
}
.cas-form .field label { font-size: 0.83rem; font-weight: 600; display: flex; align-items: center; gap: 0.35rem; }
.cas-form .field-row { display: flex; gap: 1rem; }
.cas-form .field-row .field { flex: 1; }
.cas-file-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
}
.cas-file-slot { min-width: 0; }
.cas-hint-inline { font-size: 0.78rem; font-weight: 400; color: var(--p-text-muted-color); }
.cas-filename {
    font-size: 0.8rem; color: var(--p-text-muted-color); margin-top: 0.2rem;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.cas-table { margin-bottom: 0.5rem; font-size: 0.85rem; }
.cas-debug { margin-top: 0.75rem; display: flex; flex-direction: column; gap: 0.4rem; }
.cas-debug-label { font-size: 0.78rem; color: var(--p-text-muted-color); margin: 0; }
.cas-debug-text { font-family: monospace; font-size: 0.75rem; }
.cas-confirm-btns {
    display: flex; justify-content: flex-end; gap: 0.5rem; margin-top: 1rem;
}
.cas-done {
    text-align: center; padding: 1.5rem 0;
    display: flex; flex-direction: column; align-items: center; gap: 0.75rem;
}
.cas-done-icon { font-size: 2.5rem; color: var(--p-green-500, #22c55e); }
.cas-done p { margin: 0; }

/* Zerodha section */
.section-header--first { margin-top: 0; }

.zerodha-card {
    border-radius: 12px;
    padding: 1.25rem 1.5rem;
    background: var(--p-content-background);
    border: 1px solid var(--p-content-border-color);
}

.zerodha-connecting {
    display: flex; align-items: center; gap: 0.75rem;
    padding: 0.25rem 0;
    font-size: 0.9rem; color: var(--p-text-muted-color);
}

.setup-instructions {
    margin: 0 0 1.25rem;
    padding-left: 1.5rem;
    font-size: 0.875rem;
    line-height: 1.7;
    color: var(--p-text-color);
}
.setup-instructions li { margin-bottom: 0.2rem; }
.redirect-url {
    font-size: 0.85rem;
    padding: 0.1rem 0.35rem;
    border-radius: 4px;
    background: var(--p-content-hover-background);
    font-family: monospace;
    user-select: all;
}

.setup-form {
    display: flex; flex-direction: column; gap: 0.85rem;
    max-width: 480px;
}
.setup-form .field {
    display: flex; flex-direction: column; gap: 0.35rem;
}
.setup-form .field label {
    font-size: 0.83rem; font-weight: 600;
}

.reconnect-row, .sync-row {
    display: flex; justify-content: space-between; align-items: flex-start; gap: 1.5rem;
    flex-wrap: wrap;
}
.reconnect-info, .sync-info {
    display: flex; flex-direction: column; gap: 0.3rem;
}
.reconnect-title, .sync-title {
    font-size: 0.95rem; font-weight: 600;
}
.reconnect-desc, .sync-desc {
    font-size: 0.85rem; line-height: 1.5;
}
.reconnect-btns, .sync-btns {
    display: flex; align-items: center; gap: 0.5rem; flex-shrink: 0; flex-wrap: wrap;
}
.text-muted { color: var(--p-text-muted-color); font-size: 0.82rem; }
.mt-msg { margin-top: 0.85rem; }
.sync-errors {
    margin: 0.75rem 0 0;
    padding-left: 1.25rem;
    font-size: 0.82rem;
    color: var(--p-red-400, #f87171);
}
.sync-errors li { margin-bottom: 0.2rem; }


/* Android desktop-only notice */
.desktop-only-notice {
    display: flex; align-items: flex-start; gap: 1rem;
    opacity: 0.85; padding-bottom: 0.25rem;
}
.desktop-only-icon {
    font-size: 1.6rem; color: var(--p-text-muted-color); flex-shrink: 0; margin-top: 0.1rem;
}
.desktop-only-text {
    display: flex; flex-direction: column; gap: 0.35rem;
}

@media (max-width: 700px) {
    .import-grid { grid-template-columns: 1fr; }
    .indices-row { flex-direction: column; }
    .reconnect-row, .sync-row { flex-direction: column; gap: 1rem; }
    .cas-file-row { grid-template-columns: 1fr; }
}

/* Broker CSV redirect hint */
.csv-redirect-hint {
    margin: 1rem 0 0;
    font-size: 0.82rem;
    color: var(--p-text-muted-color);
}
.link-btn {
    background: none; border: none; padding: 0; cursor: pointer;
    color: var(--p-primary-color); font-size: inherit; text-decoration: underline;
}
.link-btn:hover { color: color-mix(in srgb, var(--p-primary-color) 80%, black); }

/* Holdings CSV Dialog */
.hcsv-tabs {
    display: flex; flex-wrap: wrap; gap: 0.4rem; margin-bottom: 1.25rem;
}
.hcsv-tab {
    padding: 0.35rem 0.9rem; border-radius: 20px; font-size: 0.82rem; font-weight: 500;
    border: 1px solid var(--p-content-border-color);
    background: transparent; cursor: pointer; color: var(--p-text-muted-color);
    transition: background 0.15s, color 0.15s, border-color 0.15s;
}
.hcsv-tab:hover { color: var(--p-text-color); background: var(--p-content-hover-background); }
.hcsv-tab--active {
    background: var(--p-primary-color); color: var(--p-primary-contrast-color);
    border-color: var(--p-primary-color);
}
.hcsv-body { display: flex; flex-direction: column; gap: 1rem; }
.hcsv-upload-row { display: flex; align-items: center; gap: 0.75rem; flex-wrap: wrap; }
</style>
