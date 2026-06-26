<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { usePricesStore } from "@/stores/prices";
import { useZerodhaStore } from "@/stores/zerodha";
import { useUpstoxStore } from "@/stores/upstox";
import { useAngelOneStore } from "@/stores/angel_one";
import { parseCasPdf, mergeCasHoldings, type CasMfHolding } from "@/utils/casMfParser";
import { useAnalytics } from "@/composables/useAnalytics";

const store = usePricesStore();
const zerodha = useZerodhaStore();
const upstox = useUpstoxStore();
const angelOne = useAngelOneStore();
const { track } = useAnalytics();

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

// ── Groww CSV ───────────────────────────────────────────────────
interface GrowwRow {
    symbol: string;
    isin?: string;
    quantity: number;
    avgPrice: number;
    ltp?: number;
    exchange?: string;
}

const growwRows = ref<GrowwRow[]>([]);
const growwPreview = ref<GrowwRow[]>([]);
const growwImporting = ref(false);
const growwImportResult = ref<{ imported: number; skipped: number } | null>(null);
const growwError = ref<string | null>(null);

function parseGrowwCsv(text: string): GrowwRow[] {
    const lines = text.split('\n').filter(l => l.trim());
    if (lines.length < 2) return [];
    const headers = lines[0].split(',').map(h => h.trim().toLowerCase().replace(/\s+/g, '_'));
    const col = (names: string[]) => names.find(n => headers.includes(n)) ?? names[0];
    const symbolIdx = headers.indexOf(col(['symbol', 'ticker', 'scrip']));
    const isinIdx = headers.indexOf(col(['isin']));
    const qtyIdx = headers.indexOf(col(['qty', 'quantity']));
    const avgIdx = headers.indexOf(col(['avg_price', 'avg_cost', 'average_price', 'buy_price']));
    const ltpIdx = headers.indexOf(col(['ltp', 'current_price', 'cmp']));
    const exchIdx = headers.indexOf(col(['exchange', 'exch']));
    return lines.slice(1).map(line => {
        const cols = line.split(',').map(c => c.trim().replace(/^"|"$/g, ''));
        return {
            symbol: cols[symbolIdx] ?? '',
            isin: isinIdx >= 0 ? cols[isinIdx] : undefined,
            quantity: parseFloat(cols[qtyIdx]) || 0,
            avgPrice: parseFloat(cols[avgIdx]) || 0,
            ltp: ltpIdx >= 0 ? parseFloat(cols[ltpIdx]) : undefined,
            exchange: exchIdx >= 0 ? cols[exchIdx] : undefined,
        };
    }).filter(r => r.symbol && r.quantity > 0 && r.avgPrice > 0);
}

function onGrowwFile(event: any) {
    growwError.value = null;
    growwImportResult.value = null;
    growwRows.value = [];
    growwPreview.value = [];
    const file: File = event.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = (e) => {
        try {
            const text = e.target?.result as string;
            const parsed = parseGrowwCsv(text);
            if (parsed.length === 0) {
                growwError.value = "No valid rows found. Check the CSV format.";
                return;
            }
            growwRows.value = parsed;
            growwPreview.value = parsed.slice(0, 5);
        } catch (err: any) {
            growwError.value = `Parse error: ${err?.message ?? err}`;
        }
    };
    reader.readAsText(file);
}

async function importGroww() {
    growwImporting.value = true;
    growwError.value = null;
    try {
        const result = await invoke<{ imported: number; skipped: number }>("import_groww_csv", {
            rows: growwRows.value.map(r => ({
                symbol: r.symbol,
                isin: r.isin,
                quantity: r.quantity,
                avgPrice: r.avgPrice,
                ltp: r.ltp,
                exchange: r.exchange,
            })),
        });
        growwImportResult.value = result;
        track("groww_import_completed", { imported: result.imported, skipped: result.skipped });
    } catch (e: any) {
        growwError.value = String(e?.message ?? e);
    } finally {
        growwImporting.value = false;
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

onMounted(() => {
    zerodha.fetchStatus();
    upstox.fetchStatus();
    angelOne.fetchStatus();
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
                            <label>API Key</label>
                            <InputText
                                v-model="apiKeyInput"
                                placeholder="Your Kite API Key"
                                fluid
                            />
                        </div>
                        <div class="field">
                            <label>API Secret</label>
                            <Password
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
                                Prices refresh automatically via Yahoo Finance.
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
            </div>
        </template>
        <template v-else>
            <div class="section-header section-header--first">
                <h2>Zerodha Kite</h2>
                <Tag value="Desktop Only" severity="secondary" />
            </div>
            <div class="zerodha-card desktop-only-notice">
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
                            <label>API Key</label>
                            <InputText v-model="upstoxApiKey" placeholder="Your Upstox API Key" fluid />
                        </div>
                        <div class="field">
                            <label>API Secret</label>
                            <Password
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
            </div>
        </template>
        <template v-else>
            <div class="section-header">
                <h2>Upstox</h2>
                <Tag value="Desktop Only" severity="secondary" />
            </div>
            <div class="zerodha-card desktop-only-notice">
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
                        <label>API Key</label>
                        <InputText v-model="angelApiKey" placeholder="SmartAPI API Key" fluid />
                    </div>
                    <div class="field">
                        <label>Client Code</label>
                        <InputText v-model="angelClientId" placeholder="Your Angel One Client Code" fluid />
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
        </div>

        <!-- ══════════════════════════════════════ Groww CSV Import ══ -->
        <div class="section-header">
            <h2>Groww <span style="font-weight:400;font-size:0.9rem">(CSV Import)</span></h2>
        </div>

        <div class="zerodha-card">
            <ol class="setup-instructions">
                <li>Open Groww App or Web → Portfolio → Export → Holdings CSV</li>
                <li>Upload the downloaded CSV file below</li>
            </ol>

            <div class="setup-form">
                <div class="field">
                    <label>Holdings CSV</label>
                    <FileUpload
                        mode="basic"
                        accept=".csv"
                        :maxFileSize="5000000"
                        chooseLabel="Choose CSV"
                        customUpload
                        auto
                        @uploader="onGrowwFile"
                    />
                </div>

                <!-- Preview table -->
                <template v-if="growwPreview.length > 0">
                    <p class="reconnect-desc" style="margin:0">
                        Preview (first {{ growwPreview.length }} of {{ growwRows.length }} rows):
                    </p>
                    <DataTable :value="growwPreview" size="small" class="cas-table">
                        <Column field="symbol" header="Symbol" style="width:130px" />
                        <Column field="isin" header="ISIN" style="width:140px" />
                        <Column field="quantity" header="Qty" style="width:80px" />
                        <Column header="Avg Price" style="width:100px">
                            <template #body="{ data }">
                                ₹{{ data.avgPrice.toLocaleString("en-IN", { maximumFractionDigits: 2 }) }}
                            </template>
                        </Column>
                        <Column header="LTP" style="width:100px">
                            <template #body="{ data }">
                                {{ data.ltp != null ? "₹" + data.ltp.toLocaleString("en-IN", { maximumFractionDigits: 2 }) : "—" }}
                            </template>
                        </Column>
                        <Column field="exchange" header="Exchange" style="width:90px" />
                    </DataTable>
                </template>

                <Message v-if="growwError" severity="error">{{ growwError }}</Message>

                <div v-if="growwImportResult" class="refresh-result">
                    <Tag :value="`✓ ${growwImportResult.imported} holdings imported`" severity="success" />
                    <Tag
                        v-if="growwImportResult.skipped > 0"
                        :value="`${growwImportResult.skipped} skipped`"
                        severity="secondary"
                    />
                </div>

                <Button
                    v-if="growwRows.length > 0"
                    :label="`Import ${growwRows.length} Holdings`"
                    icon="pi pi-upload"
                    :loading="growwImporting"
                    @click="importGroww"
                />
            </div>
        </div>

        <!-- Market Pulse -->
        <div class="section-header">
            <h2>Market Pulse</h2>
            <Button
                icon="pi pi-refresh"
                label="Fetch"
                size="small"
                text
                :loading="store.indicesLoading"
                @click="store.fetchIndices()"
                v-tooltip="'Fetch live market indices from Yahoo Finance'"
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

        <!-- Price Refresh -->
        <div class="section-header">
            <h2>Price Refresh</h2>
        </div>

        <div class="refresh-grid">
            <!-- Equity -->
            <div class="refresh-card">
                <div class="refresh-card-header">
                    <div>
                        <span class="refresh-title">Equity Holdings</span>
                        <p class="refresh-desc">
                            Updates <code>current_price</code> for all NSE / BSE holdings
                            via Yahoo Finance (<code>{SYMBOL}.NS</code> / <code>.BO</code>).
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
                <div v-if="store.equityResult" class="refresh-result">
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
                <div v-if="store.mfResult" class="refresh-result">
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

        <!-- Import — coming soon -->
        <div class="section-header">
            <h2>Import</h2>
        </div>

        <div class="import-grid">
            <div class="import-card">
                <i class="pi pi-file-excel import-icon" />
                <span class="import-title">CSV Import</span>
                <span class="import-desc">
                    Import transactions from any CSV with a column-mapping wizard.
                </span>
                <Tag value="Coming Soon" />
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
.import-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 1rem; }
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
    opacity: 0.85;
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
}
</style>
