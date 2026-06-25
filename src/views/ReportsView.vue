<script setup lang="ts">
import { onMounted, computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { useReportsStore } from "@/stores/reports";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";
import { useChartColors } from "@/composables/useChartColors";
import { Line } from "vue-chartjs";
import {
    Chart as ChartJS,
    CategoryScale,
    LinearScale,
    PointElement,
    LineElement,
    Title,
    Tooltip,
    Legend,
    Filler,
} from "chart.js";

ChartJS.register(CategoryScale, LinearScale, PointElement, LineElement, Title, Tooltip, Legend, Filler);

const store = useReportsStore();
const { formatINR, formatCompact } = useCurrencyFormat();
const { textColor, mutedColor, gridColor } = useChartColors();

const activeTab = ref(0);

// ─── Net Worth History ────────────────────────────────────────
const HISTORY_PERIODS = [
    { label: "3 Months", value: 3 },
    { label: "6 Months", value: 6 },
    { label: "12 Months", value: 12 },
    { label: "2 Years", value: 24 },
];

const selectedMonths = ref(12);

function changeHistoryPeriod(months: number) {
    selectedMonths.value = months;
    store.fetchHistory(months);
}

const historyChartData = computed(() => ({
    labels: store.snapshots.map(s => s.snapshotDate),
    datasets: [
        {
            label: "Net Worth",
            data: store.snapshots.map(s => s.netWorth),
            tension: 0.3,
            fill: false,
            pointRadius: 4,
        },
        {
            label: "Total Assets",
            data: store.snapshots.map(s => s.totalAssets),
            tension: 0.3,
            fill: false,
            pointRadius: 3,
        },
        {
            label: "Total Liabilities",
            data: store.snapshots.map(s => s.totalLiabilities),
            tension: 0.3,
            fill: false,
            pointRadius: 3,
        },
    ],
}));

const historyChartOptions = computed(() => ({
    responsive: true,
    maintainAspectRatio: false,
    plugins: {
        legend: {
            position: "top" as const,
            labels: { color: textColor.value },
        },
        tooltip: {
            callbacks: {
                label: (ctx: any) => ` ${ctx.dataset.label}: ${formatINR(ctx.raw)}`,
            },
        },
    },
    scales: {
        x: { ticks: { color: mutedColor.value }, grid: { color: gridColor.value } },
        y: {
            ticks: { color: mutedColor.value, callback: (v: any) => formatCompact(Number(v)) },
            grid: { color: gridColor.value },
        },
    },
}));

// ─── Capital Gains ────────────────────────────────────────────
const METHODS = ["FIFO", "LIFO"];

function generateFYOptions(): string[] {
    const now = new Date();
    const fyStart = now.getMonth() >= 3 ? now.getFullYear() : now.getFullYear() - 1;
    return Array.from({ length: 5 }, (_, i) => {
        const start = fyStart - i;
        return `${start}-${String(start + 1).slice(-2)}`;
    });
}

const FY_OPTIONS = generateFYOptions();
const selectedFY = ref(FY_OPTIONS[0]);
const selectedMethod = ref("FIFO");

watch([selectedFY, selectedMethod], ([fy, method]) => {
    store.fetchCapitalGains(fy, method);
}, { immediate: false });

function loadGains() {
    store.fetchCapitalGains(selectedFY.value, selectedMethod.value);
}

// ─── CSV export ──────────────────────────────────────────────
function toCsvRow(cells: (string | number)[]): string {
    return cells.map(c => `"${String(c).replace(/"/g, '""')}"`).join(",");
}

async function exportNetWorthCsv() {
    const path = await save({
        defaultPath: `net-worth-history-${new Date().toISOString().slice(0,10)}.csv`,
        filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (!path) return;
    const header = toCsvRow(["Date", "Total Assets", "Total Liabilities", "Net Worth"]);
    const rows = store.snapshots.map(s =>
        toCsvRow([s.snapshotDate, s.totalAssets, s.totalLiabilities, s.netWorth])
    );
    await invoke("write_csv", { path, content: [header, ...rows].join("\n") });
}

async function exportGainsCsv() {
    if (!store.capitalGains) return;
    const path = await save({
        defaultPath: `capital-gains-${selectedFY.value}-${selectedMethod.value}.csv`,
        filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (!path) return;
    const header = toCsvRow(["Symbol", "Asset Class", "Buy Date", "Sell Date",
        "Days Held", "Quantity", "Buy Price", "Sell Price", "Gain/Loss", "Type"]);
    const rows = store.capitalGains.transactions.map(t =>
        toCsvRow([t.symbol, t.assetClass, t.buyDate, t.sellDate, t.holdingDays,
            t.quantity, t.buyPrice, t.sellPrice, t.gain.toFixed(2), t.gainType])
    );
    await invoke("write_csv", { path, content: [header, ...rows].join("\n") });
}

const LTCG_EXEMPTION = 125_000;

const stcgTax = computed(() => {
    if (!store.capitalGains) return 0;
    return Math.max(0, store.capitalGains.stcg) * 0.20;
});

const ltcgTax = computed(() => {
    if (!store.capitalGains) return 0;
    const taxable = Math.max(0, store.capitalGains.ltcg - LTCG_EXEMPTION);
    return taxable * 0.125;
});

onMounted(() => {
    store.fetchHistory(12);
    store.fetchCapitalGains(FY_OPTIONS[0], "FIFO");
});
</script>

<template>
    <div class="reports-view">
        <h1 class="page-title">Reports</h1>

        <Tabs v-model:value="activeTab">
            <TabList>
                <Tab :value="0">Net Worth History</Tab>
                <Tab :value="1">Capital Gains</Tab>
            </TabList>

            <TabPanels>
                <!-- ── Net Worth History ── -->
                <TabPanel :value="0">
                    <div class="tab-toolbar">
                        <div class="period-btns">
                            <Button
                                v-for="p in HISTORY_PERIODS"
                                :key="p.value"
                                :label="p.label"
                                size="small"
                                :outlined="selectedMonths !== p.value"
                                :text="selectedMonths !== p.value"
                                @click="changeHistoryPeriod(p.value)"
                            />
                        </div>
                        <Button
                            icon="pi pi-download"
                            label="Export CSV"
                            size="small"
                            text
                            :disabled="store.snapshots.length === 0"
                            @click="exportNetWorthCsv"
                        />
                        <Button
                            icon="pi pi-camera"
                            label="Take Snapshot"
                            size="small"
                            :loading="store.isTakingSnapshot"
                            @click="store.takeSnapshot()"
                            v-tooltip="'Saves today\'s net worth to history'"
                        />
                    </div>

                    <Message v-if="store.snapshotError" severity="error" class="mt-error">
                        {{ store.snapshotError }}
                    </Message>

                    <div v-if="store.isLoadingHistory" class="loading"><ProgressSpinner /></div>

                    <template v-else-if="store.snapshots.length > 0">
                        <div class="chart-card">
                            <div class="chart-wrap">
                                <Line :data="historyChartData" :options="historyChartOptions" />
                            </div>
                        </div>

                        <DataTable :value="store.snapshots" stripedRows class="history-table">
                            <Column field="snapshotDate" header="Date" sortable />
                            <Column field="totalAssets" header="Assets" sortable>
                                <template #body="{ data }">{{ formatINR(data.totalAssets) }}</template>
                            </Column>
                            <Column field="totalLiabilities" header="Liabilities" sortable>
                                <template #body="{ data }">{{ formatINR(data.totalLiabilities) }}</template>
                            </Column>
                            <Column field="netWorth" header="Net Worth" sortable>
                                <template #body="{ data }">{{ formatINR(data.netWorth) }}</template>
                            </Column>
                        </DataTable>
                    </template>

                    <div v-else class="empty-state">
                        <i class="pi pi-chart-line" style="font-size: 2.5rem" />
                        <p>No snapshots yet.</p>
                        <p class="hint">Click <strong>Take Snapshot</strong> to record today's net worth and start building your history.</p>
                    </div>
                </TabPanel>

                <!-- ── Capital Gains ── -->
                <TabPanel :value="1">
                    <div class="tab-toolbar">
                        <div class="gains-filters">
                            <div class="filter-field">
                                <label>Financial Year</label>
                                <Select v-model="selectedFY" :options="FY_OPTIONS" style="width:130px" @change="loadGains" />
                            </div>
                            <div class="filter-field">
                                <label>Method</label>
                                <Select v-model="selectedMethod" :options="METHODS" style="width:110px" @change="loadGains" />
                            </div>
                        </div>
                        <Button
                            icon="pi pi-download"
                            label="Export CSV"
                            size="small"
                            text
                            :disabled="!store.capitalGains || store.capitalGains.transactions.length === 0"
                            @click="exportGainsCsv"
                        />
                    </div>

                    <div v-if="store.isLoadingGains" class="loading"><ProgressSpinner /></div>

                    <template v-else-if="store.capitalGains">
                        <!-- Gain summary cards -->
                        <div class="gains-summary">
                            <div class="gains-card">
                                <span class="gains-label">Short-Term Capital Gains (STCG)</span>
                                <span class="gains-value">{{ formatINR(store.capitalGains.stcg) }}</span>
                                <span class="gains-note">Held &lt; 12 months — taxed at 20%</span>
                            </div>
                            <div class="gains-card">
                                <span class="gains-label">Long-Term Capital Gains (LTCG)</span>
                                <span class="gains-value">{{ formatINR(store.capitalGains.ltcg) }}</span>
                                <span class="gains-note">Held ≥ 12 months — 12.5% on gains above ₹1.25L</span>
                            </div>
                        </div>

                        <!-- Tax estimate panel -->
                        <div class="tax-panel">
                            <span class="tax-panel-title">Estimated Tax Liability</span>
                            <div class="tax-rows">
                                <div class="tax-row">
                                    <span>STCG Tax (20%)</span>
                                    <span>{{ formatINR(stcgTax) }}</span>
                                </div>
                                <div class="tax-row">
                                    <span>LTCG — Exempt portion (₹1.25L)</span>
                                    <span>{{ formatINR(Math.min(store.capitalGains.ltcg, 125000)) }}</span>
                                </div>
                                <div class="tax-row">
                                    <span>LTCG Tax (12.5% on taxable)</span>
                                    <span>{{ formatINR(ltcgTax) }}</span>
                                </div>
                                <div class="tax-row total-row">
                                    <span>Total Estimated Tax</span>
                                    <span>{{ formatINR(stcgTax + ltcgTax) }}</span>
                                </div>
                            </div>
                            <p class="tax-disclaimer">
                                Indicative only. Consult a CA for final ITR computation. Surcharge and cess not included.
                            </p>
                        </div>

                        <!-- Transactions table -->
                        <DataTable
                            :value="store.capitalGains.transactions"
                            stripedRows
                            sortField="sellDate"
                            :sortOrder="-1"
                            emptyMessage="No realised equity / MF sell transactions for this financial year."
                            class="gains-table"
                        >
                            <Column field="symbol" header="Symbol" sortable style="min-width:120px" />
                            <Column field="assetClass" header="Class" style="width:80px">
                                <template #body="{ data }">
                                    <Tag :value="data.assetClass.toUpperCase()" size="small" />
                                </template>
                            </Column>
                            <Column field="buyDate" header="Buy Date" sortable style="width:115px" />
                            <Column field="sellDate" header="Sell Date" sortable style="width:115px" />
                            <Column field="holdingDays" header="Days" sortable style="width:80px" />
                            <Column field="quantity" header="Qty" style="width:80px">
                                <template #body="{ data }">{{ data.quantity.toLocaleString("en-IN") }}</template>
                            </Column>
                            <Column field="buyPrice" header="Buy ₹" sortable>
                                <template #body="{ data }">{{ formatINR(data.buyPrice) }}</template>
                            </Column>
                            <Column field="sellPrice" header="Sell ₹" sortable>
                                <template #body="{ data }">{{ formatINR(data.sellPrice) }}</template>
                            </Column>
                            <Column field="gain" header="Gain / Loss" sortable>
                                <template #body="{ data }">
                                    <span :class="data.gain >= 0 ? 'gain' : 'loss'">{{ formatINR(data.gain) }}</span>
                                </template>
                            </Column>
                            <Column field="gainType" header="Type" style="width:75px">
                                <template #body="{ data }">
                                    <Tag :value="data.gainType" size="small"
                                        :severity="data.gainType === 'STCG' ? 'warn' : 'success'" />
                                </template>
                            </Column>
                        </DataTable>
                    </template>
                </TabPanel>
            </TabPanels>
        </Tabs>
    </div>
</template>

<style scoped>
.reports-view { max-width: 1100px; }
.page-title { font-size: 1.5rem; font-weight: 700; margin: 0 0 1.5rem; }

.tab-toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.75rem;
    margin-bottom: 1.25rem;
    padding-top: 1rem;
}

.period-btns { display: flex; gap: 0.5rem; flex-wrap: wrap; }

.gains-filters { display: flex; gap: 1rem; align-items: flex-end; flex-wrap: wrap; }
.filter-field { display: flex; flex-direction: column; gap: 0.35rem; }
.filter-field label { font-size: 0.8rem; font-weight: 500; }

.gains-notice { margin-bottom: 1.25rem; }
.mt-error { margin-bottom: 0.75rem; }

.loading { display: flex; justify-content: center; padding: 4rem; }

.chart-card { border-radius: 12px; padding: 1.25rem 1.5rem; margin-bottom: 1.5rem; background: var(--p-content-background); border: 1px solid var(--p-content-border-color); }
.chart-wrap { height: 300px; }

.history-table { margin-top: 0.5rem; }

.empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.75rem;
    padding: 4rem 2rem;
    text-align: center;
}
.hint { font-size: 0.9rem; }

.gains-summary { display: flex; gap: 1rem; margin-bottom: 1.5rem; flex-wrap: wrap; }
.gains-card {
    flex: 1;
    min-width: 220px;
    border-radius: 12px;
    padding: 1.25rem 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    background: var(--p-content-background);
    border: 1px solid var(--p-content-border-color);
}
.gains-label { font-size: 0.8rem; font-weight: 500; color: var(--p-text-muted-color); }
.gains-value { font-size: 1.5rem; font-weight: 700; }
.gains-note { font-size: 0.75rem; }

/* Tax estimate panel */
.tax-panel {
    border-radius: 12px;
    padding: 1.25rem 1.5rem;
    margin-bottom: 1.5rem;
    background: var(--p-content-background);
    border: 1px solid var(--p-content-border-color);
}
.tax-panel-title { font-size: 0.95rem; font-weight: 600; display: block; margin-bottom: 0.75rem; }
.tax-rows { display: flex; flex-direction: column; gap: 0.4rem; }
.tax-row {
    display: flex;
    justify-content: space-between;
    font-size: 0.9rem;
    padding: 0.3rem 0;
    border-bottom: 1px solid var(--p-content-border-color);
}
.total-row {
    font-weight: 700;
    font-size: 1rem;
    padding-top: 0.5rem;
    margin-top: 0.25rem;
    border-top: 2px solid var(--p-content-border-color);
    border-bottom: none;
}
.tax-disclaimer { font-size: 0.75rem; margin: 0.75rem 0 0; color: var(--p-text-muted-color); font-style: italic; }

.gains-table { margin-top: 0.5rem; }
</style>
