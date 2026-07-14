<script setup lang="ts">
import { onMounted, computed } from "vue";
import { usePortfolioStore } from "@/stores/portfolio";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";
import { useChartColors } from "@/composables/useChartColors";
import { useMilestoneCheck } from "@/composables/useMilestoneCheck";
import { useGoalCheck } from "@/composables/useGoalCheck";
import { useGamificationStore } from "@/stores/gamification";
import { useInsightsStore } from "@/stores/insights";
import { useHealthCheck } from "@/composables/useHealthCheck";
import { Doughnut } from "vue-chartjs";
import { Chart as ChartJS, ArcElement, Tooltip, Legend } from "chart.js";

ChartJS.register(ArcElement, Tooltip, Legend);

const isGamified = import.meta.env.VITE_GAMIFICATION === "true";

const portfolio = usePortfolioStore();
const { formatCompact, formatPercent } = useCurrencyFormat();
const { textColor, PALETTE } = useChartColors();
const { checkMilestones } = useMilestoneCheck();
const { checkGoals } = useGoalCheck();
const gamification = useGamificationStore();
const insights = useInsightsStore();
const { runHealthCheck } = useHealthCheck();

onMounted(async () => {
    await portfolio.fetchAll();
    if (portfolio.netWorth) {
        checkMilestones(portfolio.netWorth.netWorth);
        checkGoals(portfolio.netWorth.totalAssets);
    }
    if (isGamified) {
        await gamification.fetch();
    }
    // Financial Health Score (core — score always computes; XP/badges only if gamified).
    await runHealthCheck();
    // Behavioural nudges derived from the just-refreshed data.
    await insights.fetch();
});

const chartData = computed(() => ({
    labels: portfolio.allocation.map((a) => a.label),
    datasets: [{
        data: portfolio.allocation.map((a) => a.value),
        backgroundColor: PALETTE,
        borderWidth: 2,
    }],
}));

const chartOptions = computed(() => ({
    responsive: true,
    maintainAspectRatio: false,
    plugins: {
        legend: {
            position: "right" as const,
            labels: { color: textColor.value, boxWidth: 14, padding: 12 },
        },
        tooltip: {
            callbacks: {
                label(ctx: any) {
                    const item = portfolio.allocation[ctx.dataIndex];
                    return ` ${item.label}: ${formatPercent(item.percent)}`;
                },
            },
        },
    },
}));
</script>

<template>
    <div class="dashboard">
        <h1 class="page-title">Dashboard</h1>

        <div v-if="portfolio.isLoading" class="loading-state">
            <ProgressSpinner />
        </div>

        <template v-else>
            <div class="net-worth-card">
                <div class="nw-section">
                    <span class="nw-label">Net Worth</span>
                    <span class="nw-value">
                        {{ portfolio.netWorth ? formatCompact(portfolio.netWorth.netWorth) : "₹0" }}
                    </span>
                </div>
                <div class="nw-breakdown">
                    <div class="nw-item">
                        <span class="nw-item-label">Total Assets</span>
                        <span class="nw-item-value">
                            {{ portfolio.netWorth ? formatCompact(portfolio.netWorth.totalAssets) : "₹0" }}
                        </span>
                    </div>
                    <div class="nw-item">
                        <span class="nw-item-label">Total Liabilities</span>
                        <span class="nw-item-value">
                            {{ portfolio.netWorth ? formatCompact(portfolio.netWorth.totalLiabilities) : "₹0" }}
                        </span>
                    </div>
                </div>
            </div>

            <InsightsFeed />

            <FinancialHealthCard />

            <GamificationWidget v-if="isGamified" />

            <div class="card chart-card" v-if="portfolio.allocation.length > 0">
                <h3>Asset Allocation</h3>
                <div class="chart-wrap">
                    <Doughnut :data="chartData" :options="chartOptions" />
                </div>
            </div>
            <div class="card empty-state" v-else>
                <i class="pi pi-chart-pie" style="font-size: 2rem" />
                <p>No holdings yet. Add your first asset in <RouterLink to="/portfolio">Portfolio</RouterLink>.</p>
            </div>
        </template>
    </div>
</template>

<style scoped>
.dashboard {
    max-width: 900px;
}

.page-title {
    font-size: 1.5rem;
    font-weight: 700;
    margin: 0 0 1.5rem;
}

.loading-state {
    display: flex;
    justify-content: center;
    padding: 3rem;
}

.net-worth-card {
    border-radius: 12px;
    padding: 1.5rem 2rem;
    margin-bottom: 1.5rem;
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 1rem;
    background: var(--p-content-background);
    border: 1px solid var(--p-content-border-color);
}

.nw-label {
    font-size: 0.85rem;
    display: block;
    margin-bottom: 0.25rem;
    color: var(--p-text-muted-color);
}

.nw-value {
    font-size: 2.2rem;
    font-weight: 800;
    display: block;
}

.nw-breakdown {
    display: flex;
    gap: 2rem;
}

.nw-item {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
}

.nw-item-label {
    font-size: 0.8rem;
}

.nw-item-value {
    font-size: 1.2rem;
    font-weight: 700;
}

.card {
    border-radius: 12px;
    padding: 1.5rem;
    margin-bottom: 1.5rem;
    background: var(--p-content-background);
    border: 1px solid var(--p-content-border-color);
}

.nw-item-label {
    color: var(--p-text-muted-color);
}

.chart-card h3 {
    margin: 0 0 1rem;
    font-size: 1rem;
    font-weight: 600;
}

.chart-wrap {
    height: 280px;
}

.empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.75rem;
    padding: 3rem;
    text-align: center;
}

@media (max-width: 639px) {
    .net-worth-card {
        padding: 1rem;
        flex-direction: column;
        align-items: flex-start;
    }

    .nw-value {
        font-size: 1.75rem;
    }

    .nw-breakdown {
        flex-direction: column;
        gap: 0.75rem;
        width: 100%;
    }

    .chart-wrap {
        height: 200px;
    }
}
</style>
