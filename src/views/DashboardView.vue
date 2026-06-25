<script setup lang="ts">
import { onMounted } from "vue";
import { usePortfolioStore } from "@/stores/portfolio";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";
import { Doughnut } from "vue-chartjs";
import { Chart as ChartJS, ArcElement, Tooltip, Legend } from "chart.js";

ChartJS.register(ArcElement, Tooltip, Legend);

const portfolio = usePortfolioStore();
const { formatCompact, formatPercent } = useCurrencyFormat();

onMounted(() => portfolio.fetchAll());

const chartData = {
    get labels() {
        return portfolio.allocation.map((a) => a.label);
    },
    get datasets() {
        return [{
            data: portfolio.allocation.map((a) => a.value),
        }];
    },
};

const chartOptions = {
    responsive: true,
    maintainAspectRatio: false,
    plugins: {
        legend: { position: "right" as const },
        tooltip: {
            callbacks: {
                label(ctx: any) {
                    const item = portfolio.allocation[ctx.dataIndex];
                    return ` ${item.label}: ${formatPercent(item.percent)}`;
                },
            },
        },
    },
};
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
}

.nw-label {
    font-size: 0.85rem;
    display: block;
    margin-bottom: 0.25rem;
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
</style>
