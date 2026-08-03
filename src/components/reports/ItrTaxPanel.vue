<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { Bar, Doughnut } from "vue-chartjs";
import {
    ArcElement,
    BarElement,
    CategoryScale,
    Chart as ChartJS,
    Legend,
    LinearScale,
    Title,
    Tooltip,
} from "chart.js";
import { useItrStore, type ItrReturn } from "@/stores/itr";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";
import { useChartColors } from "@/composables/useChartColors";
import ItrImportDialog from "@/components/reports/ItrImportDialog.vue";

ChartJS.register(CategoryScale, LinearScale, BarElement, ArcElement, Title, Tooltip, Legend);

const store = useItrStore();
const { formatINR, chartTick } = useCurrencyFormat();
const { textColor, mutedColor, gridColor } = useChartColors();

const dialogVisible = ref(false);
const editing = ref<ItrReturn | null>(null);
const selectedYear = ref<string | null>(null);

onMounted(async () => {
    await store.fetchAll();
    selectedYear.value = store.summary?.latestAssessmentYear ?? null;
});

// Keep the donut's year valid after imports and deletes.
watch(
    () => store.returns.map((r) => r.assessmentYear).join(","),
    () => {
        if (!store.returns.length) {
            selectedYear.value = null;
        } else if (!store.returns.some((r) => r.assessmentYear === selectedYear.value)) {
            selectedYear.value = store.returns[store.returns.length - 1].assessmentYear;
        }
    },
);

const years = computed(() => store.returns.map((r) => r.assessmentYear));

const yearOptions = computed(() =>
    store.returns.map((r) => ({ label: `AY ${r.assessmentYear}`, value: r.assessmentYear })),
);

const selected = computed(
    () => store.returns.find((r) => r.assessmentYear === selectedYear.value) ?? null,
);

const effectiveRate = computed(() => store.summary?.averageEffectiveRate ?? 0);

// ─── charts ───────────────────────────────────────────────────

const INCOME_COLORS = ["#42A5F5", "#66BB6A", "#FFA726", "#AB47BC", "#26C6DA", "#EF5350"];

const taxPaidData = computed(() => ({
    labels: years.value.map((y) => `AY ${y}`),
    datasets: [
        {
            label: "TDS",
            backgroundColor: "#42A5F5",
            data: store.returns.map((r) => r.tdsPaid),
        },
        {
            label: "Advance tax",
            backgroundColor: "#66BB6A",
            data: store.returns.map((r) => r.advanceTaxPaid),
        },
        {
            label: "Self-assessment",
            backgroundColor: "#FFA726",
            data: store.returns.map((r) => r.selfAssessmentTaxPaid),
        },
        {
            label: "TCS",
            backgroundColor: "#AB47BC",
            data: store.returns.map((r) => r.tcsPaid),
        },
    ],
}));

const incomeTrendData = computed(() => ({
    labels: years.value.map((y) => `AY ${y}`),
    datasets: [
        { label: "Salary", backgroundColor: INCOME_COLORS[0], data: store.returns.map((r) => r.salaryIncome) },
        { label: "House property", backgroundColor: INCOME_COLORS[1], data: store.returns.map((r) => r.housePropertyIncome) },
        { label: "STCG", backgroundColor: INCOME_COLORS[2], data: store.returns.map((r) => r.capitalGainsStcg) },
        { label: "LTCG", backgroundColor: INCOME_COLORS[3], data: store.returns.map((r) => r.capitalGainsLtcg) },
        { label: "Other sources", backgroundColor: INCOME_COLORS[4], data: store.returns.map((r) => r.otherSourcesIncome) },
        { label: "Business", backgroundColor: INCOME_COLORS[5], data: store.returns.map((r) => r.businessIncome) },
    ],
}));

const distributionData = computed(() => {
    const r = selected.value;
    const parts = r
        ? [
              { label: "Salary", value: r.salaryIncome },
              { label: "House property", value: r.housePropertyIncome },
              { label: "STCG", value: r.capitalGainsStcg },
              { label: "LTCG", value: r.capitalGainsLtcg },
              { label: "Other sources", value: r.otherSourcesIncome },
              { label: "Business", value: r.businessIncome },
          ].filter((p) => p.value > 0)
        : [];
    return {
        labels: parts.map((p) => p.label),
        datasets: [{ backgroundColor: INCOME_COLORS.slice(0, parts.length), data: parts.map((p) => p.value) }],
    };
});

const stackedOptions = computed(() => ({
    responsive: true,
    maintainAspectRatio: false,
    plugins: {
        legend: { labels: { color: textColor.value } },
        tooltip: {
            callbacks: {
                label: (ctx: any) => `${ctx.dataset.label}: ${formatINR(ctx.parsed.y)}`,
            },
        },
    },
    scales: {
        x: { stacked: true, ticks: { color: mutedColor.value }, grid: { color: gridColor.value } },
        y: {
            stacked: true,
            ticks: { color: mutedColor.value, callback: chartTick.value },
            grid: { color: gridColor.value },
        },
    },
}));

const donutOptions = computed(() => ({
    responsive: true,
    maintainAspectRatio: false,
    plugins: {
        legend: { position: "bottom" as const, labels: { color: textColor.value } },
        tooltip: {
            callbacks: {
                label: (ctx: any) => `${ctx.label}: ${formatINR(ctx.parsed)}`,
            },
        },
    },
}));

// ─── actions ──────────────────────────────────────────────────

function openImport() {
    editing.value = null;
    dialogVisible.value = true;
}

function openEdit(row: ItrReturn) {
    editing.value = row;
    dialogVisible.value = true;
}

async function onDelete(row: ItrReturn) {
    if (row.id == null) return;
    await store.remove(row.id);
}

async function onSaved() {
    await store.fetchAll();
    selectedYear.value = store.summary?.latestAssessmentYear ?? selectedYear.value;
}
</script>

<template>
    <div class="itr-panel">
        <div class="tab-toolbar">
            <Button label="Import ITR" icon="pi pi-file-import" @click="openImport" />
            <span class="toolbar-note">ITR-2 / ITR-3 PDF or JSON, parsed on this device</span>
        </div>

        <div v-if="store.isLoading" class="empty">
            <ProgressSpinner style="width: 2.5rem; height: 2.5rem" />
        </div>

        <div v-else-if="!store.returns.length" class="empty">
            <i class="pi pi-file-pdf empty-icon" />
            <p>No returns imported yet.</p>
            <Button label="Import ITR" icon="pi pi-file-import" @click="openImport" />
        </div>

        <template v-else>
            <div class="cards">
                <div class="card">
                    <span class="card-label">Latest AY ({{ store.summary?.latestAssessmentYear }})</span>
                    <span class="card-value">{{ formatINR(store.summary?.latestTotalTaxLiability ?? 0) }}</span>
                    <span class="card-sub">tax liability</span>
                </div>
                <div class="card">
                    <span class="card-label">Effective tax rate</span>
                    <span class="card-value">{{ effectiveRate }}%</span>
                    <span class="card-sub">lifetime liability / gross income</span>
                </div>
                <div class="card">
                    <span class="card-label">Lifetime tax paid</span>
                    <span class="card-value">{{ formatINR(store.summary?.lifetimeTaxPaid ?? 0) }}</span>
                    <span class="card-sub">{{ store.summary?.returnsCount }} return(s)</span>
                </div>
                <div class="card">
                    <span class="card-label">Lifetime gross income</span>
                    <span class="card-value">{{ formatINR(store.summary?.lifetimeGrossIncome ?? 0) }}</span>
                    <span class="card-sub">as filed</span>
                </div>
            </div>

            <div class="chart-block">
                <h3>Tax paid by assessment year</h3>
                <div class="chart-wrap">
                    <Bar :data="taxPaidData" :options="stackedOptions" />
                </div>
            </div>

            <div class="chart-row">
                <div class="chart-block">
                    <div class="chart-head">
                        <h3>Income distribution</h3>
                        <Select
                            v-model="selectedYear"
                            :options="yearOptions"
                            option-label="label"
                            option-value="value"
                            class="year-select"
                        />
                    </div>
                    <div class="chart-wrap">
                        <Doughnut :data="distributionData" :options="donutOptions" />
                    </div>
                </div>

                <div class="chart-block">
                    <h3>Income heads across years</h3>
                    <div class="chart-wrap">
                        <Bar :data="incomeTrendData" :options="stackedOptions" />
                    </div>
                </div>
            </div>

            <DataTable :value="store.returns" striped-rows class="itr-table">
                <Column field="assessmentYear" header="AY" sortable style="width: 100px" />
                <Column field="formType" header="Form" style="width: 90px" />
                <Column field="grossTotalIncome" header="Gross income" sortable>
                    <template #body="{ data }">{{ formatINR(data.grossTotalIncome) }}</template>
                </Column>
                <Column field="totalIncome" header="Taxable" sortable>
                    <template #body="{ data }">{{ formatINR(data.totalIncome) }}</template>
                </Column>
                <Column field="totalTaxLiability" header="Tax liability" sortable>
                    <template #body="{ data }">{{ formatINR(data.totalTaxLiability) }}</template>
                </Column>
                <Column field="totalTaxPaid" header="Tax paid" sortable>
                    <template #body="{ data }">{{ formatINR(data.totalTaxPaid) }}</template>
                </Column>
                <Column field="refundDue" header="Refund" sortable>
                    <template #body="{ data }">{{ formatINR(data.refundDue) }}</template>
                </Column>
                <Column header="" style="width: 110px">
                    <template #body="{ data }">
                        <Button icon="pi pi-pencil" text rounded @click="openEdit(data)" />
                        <Button icon="pi pi-trash" text rounded severity="danger" @click="onDelete(data)" />
                    </template>
                </Column>
            </DataTable>
        </template>

        <ItrImportDialog v-model:visible="dialogVisible" :editing="editing" @saved="onSaved" />
    </div>
</template>

<style scoped>
.itr-panel {
    display: flex;
    flex-direction: column;
    gap: 1rem;
}
.tab-toolbar {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex-wrap: wrap;
}
.toolbar-note {
    color: var(--p-text-muted-color);
    font-size: 0.8rem;
}
.empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.75rem;
    padding: 2.5rem 0;
    color: var(--p-text-muted-color);
}
.empty-icon {
    font-size: 2rem;
}
.cards {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 0.75rem;
}
.card {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    padding: 0.9rem 1rem;
    border-radius: 10px;
    background: var(--p-content-background);
    border: 1px solid var(--p-content-border-color);
}
.card-label {
    font-size: 0.78rem;
    color: var(--p-text-muted-color);
}
.card-value {
    font-size: 1.25rem;
    font-weight: 600;
}
.card-sub {
    font-size: 0.72rem;
    color: var(--p-text-muted-color);
}
.chart-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
}
.chart-block h3 {
    margin: 0 0 0.5rem;
    font-size: 0.95rem;
}
.chart-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
}
.year-select {
    min-width: 9rem;
}
.chart-wrap {
    height: 280px;
}
.itr-table {
    margin-top: 0.5rem;
}
@media (max-width: 639px) {
    .cards,
    .chart-row {
        grid-template-columns: 1fr;
    }
    .chart-wrap {
        height: 240px;
    }
}
</style>
