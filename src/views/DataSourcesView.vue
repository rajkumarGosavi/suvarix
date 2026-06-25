<script setup lang="ts">
import { usePricesStore } from "@/stores/prices";
const store = usePricesStore();

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
            <div class="import-card">
                <i class="pi pi-file import-icon" />
                <span class="import-title">MF Central CAS</span>
                <span class="import-desc">
                    Import your consolidated account statement from MF Central.
                </span>
                <Tag value="Coming Soon" />
            </div>
        </div>
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
.indices-updated { font-size: 0.78rem; margin: 0.5rem 0 0; }

/* Price refresh cards */
.refresh-grid { display: flex; flex-direction: column; gap: 1rem; }
.refresh-card { border-radius: 12px; padding: 1.25rem 1.5rem; background: var(--p-content-background); border: 1px solid var(--p-content-border-color); }
.refresh-card-header { display: flex; justify-content: space-between; align-items: flex-start; gap: 1rem; }
.refresh-title { font-size: 1rem; font-weight: 600; display: block; margin-bottom: 0.35rem; }
.refresh-desc { font-size: 0.83rem; margin: 0; }
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

@media (max-width: 700px) {
    .import-grid { grid-template-columns: 1fr; }
    .indices-row { flex-direction: column; }
}
</style>
