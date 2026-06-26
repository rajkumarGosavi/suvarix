<script setup lang="ts">
import { onMounted, ref } from "vue";
import { usePortfolioStore } from "@/stores/portfolio";
import EquityPanel from "@/components/portfolio/EquityPanel.vue";
import MfPanel from "@/components/portfolio/MfPanel.vue";
import FdPanel from "@/components/portfolio/FdPanel.vue";
import BondsPanel from "@/components/portfolio/BondsPanel.vue";
import PpfEpfPanel from "@/components/portfolio/PpfEpfPanel.vue";
import RealEstatePanel from "@/components/portfolio/RealEstatePanel.vue";
import GoldPanel from "@/components/portfolio/GoldPanel.vue";
import CryptoPanel from "@/components/portfolio/CryptoPanel.vue";
import InsurancePanel from "@/components/portfolio/InsurancePanel.vue";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";

const portfolio = usePortfolioStore();
const { formatCompact } = useCurrencyFormat();
const activeTab = ref(0);

onMounted(() => portfolio.fetchAll());

const tabs = [
    { label: "Equity", component: EquityPanel },
    { label: "Mutual Funds", component: MfPanel },
    { label: "FD/RD", component: FdPanel },
    { label: "Bonds", component: BondsPanel },
    { label: "PPF/EPF/NPS", component: PpfEpfPanel },
    { label: "Real Estate", component: RealEstatePanel },
    { label: "Gold", component: GoldPanel },
    { label: "Crypto", component: CryptoPanel },
    { label: "Insurance", component: InsurancePanel },
];
</script>

<template>
    <div class="portfolio-view">
        <div class="page-header">
            <h1 class="page-title">Portfolio</h1>
            <div class="header-stats" v-if="portfolio.netWorth">
                <div class="stat">
                    <span class="stat-label">Total Assets</span>
                    <span class="stat-value">{{ formatCompact(portfolio.netWorth.totalAssets) }}</span>
                </div>
            </div>
        </div>

        <div v-if="portfolio.isLoading" class="loading">
            <ProgressSpinner />
        </div>

        <Tabs v-else v-model:value="activeTab">
            <TabList>
                <Tab v-for="(tab, i) in tabs" :key="tab.label" :value="i">{{ tab.label }}</Tab>
            </TabList>
            <TabPanels>
                <TabPanel v-for="(tab, i) in tabs" :key="tab.label" :value="i">
                    <component :is="tab.component" />
                </TabPanel>
            </TabPanels>
        </Tabs>
    </div>
</template>

<style scoped>
.portfolio-view { max-width: 1200px; }
.page-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.5rem; flex-wrap: wrap; gap: 1rem; }
.page-title { font-size: 1.5rem; font-weight: 700; margin: 0; }
.header-stats { display: flex; gap: 2rem; }
.stat { display: flex; flex-direction: column; gap: 0.2rem; }
.stat-label { font-size: 0.75rem; }
.stat-value { font-size: 1.1rem; font-weight: 700; }
.loading { display: flex; justify-content: center; padding: 4rem; }
</style>
