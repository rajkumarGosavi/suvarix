<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { usePricesStore } from "@/stores/prices";
import { useZerodhaStore } from "@/stores/zerodha";
import { parseCasPdf, type CasMfHolding } from "@/utils/casMfParser";

const store = usePricesStore();
const zerodha = useZerodhaStore();

const apiKeyInput = ref("");
const apiSecretInput = ref("");

async function saveAndConnect() {
    try {
        await zerodha.saveConfig(apiKeyInput.value.trim(), apiSecretInput.value.trim());
        apiSecretInput.value = "";
        await zerodha.connect();
    } catch {
        // error shown via zerodha.error
    }
}

onMounted(() => zerodha.fetchStatus());

// ── CAS import state ────────────────────────────────────────────
const casOpen = ref(false);
const casStep = ref(1);
const casFile = ref<File | null>(null);
const casPassword = ref("");
const casParsing = ref(false);
const casImporting = ref(false);
const casParsed = ref<CasMfHolding[]>([]);
const casError = ref<string | null>(null);
const casImportResult = ref<{ imported: number; skipped: number } | null>(null);

function casReset() {
    casStep.value = 1;
    casFile.value = null;
    casPassword.value = "";
    casParsed.value = [];
    casError.value = null;
    casImportResult.value = null;
}

function onCasFile(event: any) {
    casFile.value = event.files?.[0] ?? null;
}

async function parseCas() {
    if (!casFile.value) return;
    casParsing.value = true;
    casError.value = null;
    try {
        const buffer = await casFile.value.arrayBuffer();
        const holdings = await parseCasPdf(buffer, casPassword.value.trim());
        if (holdings.length === 0) {
            casError.value =
                "No holdings found. Check the password or try a different CAS PDF.";
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
                    Password = PAN (uppercase) + date of birth as
                    <code>DDMMYYYY</code>. Example:
                    <code>ABCDE1234F01011990</code>
                </p>
                <div class="cas-form">
                    <div class="field">
                        <label>CAS PDF File</label>
                        <FileUpload
                            mode="basic"
                            accept=".pdf"
                            :maxFileSize="20000000"
                            chooseLabel="Choose PDF"
                            customUpload
                            auto
                            @uploader="onCasFile"
                        />
                        <span v-if="casFile" class="cas-filename">{{ casFile.name }}</span>
                    </div>
                    <div class="field">
                        <label>Password</label>
                        <Password
                            v-model="casPassword"
                            :feedback="false"
                            toggleMask
                            fluid
                            placeholder="PAN + DOB e.g. ABCDE1234F01011990"
                        />
                    </div>
                    <Message v-if="casError" severity="error">{{ casError }}</Message>
                    <Button
                        label="Parse PDF"
                        icon="pi pi-search"
                        :loading="casParsing"
                        :disabled="!casFile"
                        @click="parseCas"
                    />
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
.cas-form .field label { font-size: 0.83rem; font-weight: 600; }
.cas-filename {
    font-size: 0.8rem; color: var(--p-text-muted-color); margin-top: 0.2rem;
}
.cas-table { margin-bottom: 0.5rem; font-size: 0.85rem; }
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

@media (max-width: 700px) {
    .import-grid { grid-template-columns: 1fr; }
    .indices-row { flex-direction: column; }
    .reconnect-row, .sync-row { flex-direction: column; gap: 1rem; }
}
</style>
