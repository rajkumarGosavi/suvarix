<script setup lang="ts">
import { onMounted, computed, ref } from "vue";
import { useBudgetStore } from "@/stores/budget";
import { useCategoriesStore } from "@/stores/categories";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";
import { useChartColors } from "@/composables/useChartColors";
import CategoryManagerDialog from "@/components/CategoryManagerDialog.vue";
import { Bar } from "vue-chartjs";
import {
    Chart as ChartJS,
    CategoryScale,
    LinearScale,
    BarElement,
    Title,
    Tooltip,
    Legend,
} from "chart.js";

ChartJS.register(CategoryScale, LinearScale, BarElement, Title, Tooltip, Legend);

const store = useBudgetStore();
const categoriesStore = useCategoriesStore();
const { formatINR, formatCompact } = useCurrencyFormat();
const { textColor, mutedColor, gridColor } = useChartColors();

const PERIODS = [
    { label: "This Month", value: "this_month" },
    { label: "Last Month", value: "last_month" },
    { label: "All Time", value: "all" },
];

const showCategoryManager = ref(false);
const selectedPeriod = ref("this_month");

function changePeriod(value: string) {
    selectedPeriod.value = value;
    store.fetchAll(value);
}

// Chart — reverse trend so oldest is on the left
const trendChartData = computed(() => {
    const rows = [...store.monthlyTrend].reverse();
    return {
        labels: rows.map(t => t.month),
        datasets: [
            { label: "Income", data: rows.map(t => t.income), backgroundColor: "#10b981", borderRadius: 4 },
            { label: "Expense", data: rows.map(t => t.expense), backgroundColor: "#ef4444", borderRadius: 4 },
        ],
    };
});

const trendChartOptions = computed(() => ({
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
        x: {
            ticks: { color: mutedColor.value },
            grid: { color: gridColor.value },
            border: { color: gridColor.value },
        },
        y: {
            ticks: { color: mutedColor.value, callback: (v: any) => formatCompact(Number(v)) },
            grid: { color: gridColor.value },
            border: { color: gridColor.value },
        },
    },
}));

// Budget dialog
const showBudgetDialog = ref(false);
const budgetLoading = ref(false);
const budgetForm = ref({ category: "", monthlyLimit: 0 });

function openSetBudget(category?: string, limit?: number) {
    budgetForm.value = {
        category: category ?? categoriesStore.names[0] ?? "",
        monthlyLimit: limit ?? 0,
    };
    showBudgetDialog.value = true;
}

async function saveBudget() {
    budgetLoading.value = true;
    try {
        await store.setBudget(budgetForm.value.category, budgetForm.value.monthlyLimit);
        showBudgetDialog.value = false;
    } finally {
        budgetLoading.value = false;
    }
}

const incomeSummary = computed(() =>
    store.categorySummary.filter(c => c.txType === "income")
);

const expenseSummary = computed(() =>
    store.categorySummary.filter(c => c.txType === "expense")
);

const totalIncome = computed(() =>
    incomeSummary.value.reduce((s, c) => s + c.total, 0)
);

const totalExpense = computed(() =>
    expenseSummary.value.reduce((s, c) => s + c.total, 0)
);

onMounted(() => {
    store.fetchAll();
    categoriesStore.fetchCategories();
});
</script>

<template>
    <div class="ie-view">
        <div class="page-header">
            <h1 class="page-title">Income &amp; Expenses</h1>
            <Select
                v-model="selectedPeriod"
                :options="PERIODS"
                optionLabel="label"
                optionValue="value"
                @change="changePeriod(selectedPeriod)"
                style="width:160px"
            />
        </div>

        <div v-if="store.isLoading" class="loading"><ProgressSpinner /></div>

        <template v-else>
            <!-- Summary cards -->
            <div class="summary-row">
                <div class="summary-card">
                    <span class="summary-label">Total Income</span>
                    <span class="summary-value">{{ formatINR(totalIncome) }}</span>
                </div>
                <div class="summary-card">
                    <span class="summary-label">Total Expenses</span>
                    <span class="summary-value">{{ formatINR(totalExpense) }}</span>
                </div>
                <div class="summary-card">
                    <span class="summary-label">Net Savings</span>
                    <span class="summary-value">{{ formatINR(totalIncome - totalExpense) }}</span>
                </div>
            </div>

            <!-- Monthly trend bar chart -->
            <div class="card" v-if="store.monthlyTrend.length > 0">
                <h3>Monthly Trend</h3>
                <div class="chart-wrap">
                    <Bar :data="trendChartData" :options="trendChartOptions" />
                </div>
            </div>

            <!-- Category breakdown split into income / expenses -->
            <div class="two-col" v-if="store.categorySummary.length > 0">
                <div class="card">
                    <h3>Income by Category</h3>
                    <DataTable :value="incomeSummary" stripedRows emptyMessage="No income transactions.">
                        <Column field="category" header="Category" />
                        <Column field="total" header="Amount">
                            <template #body="{ data }">{{ formatINR(data.total) }}</template>
                        </Column>
                        <Column field="count" header="Txns" style="width:60px" />
                    </DataTable>
                </div>
                <div class="card">
                    <h3>Expenses by Category</h3>
                    <DataTable :value="expenseSummary" stripedRows emptyMessage="No expense transactions.">
                        <Column field="category" header="Category" />
                        <Column field="total" header="Amount">
                            <template #body="{ data }">{{ formatINR(data.total) }}</template>
                        </Column>
                        <Column field="count" header="Txns" style="width:60px" />
                    </DataTable>
                </div>
            </div>
            <div class="card empty-state" v-else>
                <i class="pi pi-chart-bar" style="font-size: 2rem" />
                <p>
                    No categorized transactions yet. Add transactions with categories in
                    <RouterLink to="/transactions">Transactions</RouterLink>.
                </p>
            </div>

            <!-- Budget Manager -->
            <div class="section-header">
                <h2>Budget Manager</h2>
                <Button icon="pi pi-plus" label="Set Budget" size="small" @click="openSetBudget()" />
            </div>

            <div v-if="store.budgetStatus.length > 0">
                <DataTable :value="store.budgetStatus" stripedRows>
                    <Column field="category" header="Category" style="width:140px" />
                    <Column header="Progress">
                        <template #body="{ data }">
                            <div class="budget-progress">
                                <ProgressBar
                                    :value="Math.min(data.percentUsed, 100)"
                                    style="height:8px"
                                    :pt="{ value: { style: data.percentUsed > 100 ? 'background:var(--p-red-500)' : '' } }"
                                />
                                <span class="budget-amounts" :class="data.percentUsed > 100 ? 'loss' : ''">
                                    {{ formatINR(data.spent) }} of {{ formatINR(data.monthlyLimit) }}
                                    <span v-if="data.percentUsed > 100"> (+{{ (data.percentUsed - 100).toFixed(0) }}%)</span>
                                </span>
                            </div>
                        </template>
                    </Column>
                    <Column field="remaining" header="Remaining" style="width:130px">
                        <template #body="{ data }">{{ formatINR(data.remaining) }}</template>
                    </Column>
                    <Column header="" style="width:60px">
                        <template #body="{ data }">
                            <Button icon="pi pi-pencil" text size="small"
                                @click="openSetBudget(data.category, data.monthlyLimit)" />
                        </template>
                    </Column>
                </DataTable>
            </div>
            <p v-else class="no-budgets">No budgets set. Click "Set Budget" to add monthly limits per category.</p>
        </template>
    </div>

    <!-- Set Budget Dialog -->
    <Dialog v-model:visible="showBudgetDialog" header="Set Monthly Budget" modal style="width:380px">
        <form @submit.prevent="saveBudget" class="dialog-form">
            <div class="field">
                <label>Category *</label>
                <div class="category-field-row">
                    <Select v-model="budgetForm.category" :options="categoriesStore.names" class="w-full" required />
                    <Button icon="pi pi-cog" text aria-label="Manage categories" v-tooltip="'Manage categories'" @click="showCategoryManager = true" />
                </div>
            </div>
            <div class="field">
                <label>Monthly Limit (₹) *</label>
                <InputNumber v-model="budgetForm.monthlyLimit" :min="1" :minFractionDigits="2" class="w-full" required />
            </div>
            <div class="dialog-footer">
                <Button label="Cancel" outlined @click="showBudgetDialog = false" />
                <Button type="submit" label="Save Budget" :loading="budgetLoading" />
            </div>
        </form>
    </Dialog>

    <CategoryManagerDialog v-model:visible="showCategoryManager" />
</template>

<style scoped>
.ie-view { max-width: 1100px; }
.page-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.5rem; flex-wrap: wrap; gap: 1rem; }
.page-title { font-size: 1.5rem; font-weight: 700; margin: 0; }
.loading { display: flex; justify-content: center; padding: 4rem; }

.summary-row { display: flex; gap: 1rem; margin-bottom: 1.5rem; flex-wrap: wrap; }
.summary-card { flex: 1; min-width: 160px; border-radius: 12px; padding: 1.25rem 1.5rem; display: flex; flex-direction: column; gap: 0.35rem; background: var(--p-content-background); border: 1px solid var(--p-content-border-color); }
.summary-label { font-size: 0.8rem; color: var(--p-text-muted-color); }
.summary-value { font-size: 1.4rem; font-weight: 700; }

.card { border-radius: 12px; padding: 1.25rem 1.5rem; margin-bottom: 1.5rem; background: var(--p-content-background); border: 1px solid var(--p-content-border-color); }
.card h3 { margin: 0 0 1rem; font-size: 1rem; font-weight: 600; }
.chart-wrap { height: 260px; }

.two-col { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; margin-bottom: 1.5rem; }

.empty-state { display: flex; flex-direction: column; align-items: center; gap: 0.75rem; padding: 3rem; text-align: center; }

.section-header { display: flex; justify-content: space-between; align-items: center; margin: 0 0 0.75rem; }
.section-header h2 { margin: 0; font-size: 1.1rem; }

.budget-progress { display: flex; flex-direction: column; gap: 0.35rem; }
.budget-amounts { font-size: 0.78rem; }

.no-budgets { font-size: 0.9rem; margin-top: 0.5rem; }

.dialog-form { display: flex; flex-direction: column; gap: 1rem; padding: 0.5rem 0; }
.field { display: flex; flex-direction: column; gap: 0.4rem; }
.category-field-row { display: flex; gap: 0.4rem; align-items: center; }
.category-field-row .p-select { flex: 1; }
label { font-size: 0.85rem; font-weight: 500; }
.dialog-footer { display: flex; justify-content: flex-end; gap: 0.75rem; margin-top: 0.5rem; }

@media (max-width: 700px) {
    .two-col { grid-template-columns: 1fr; }
}
</style>
