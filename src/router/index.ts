import { createRouter, createWebHashHistory, type RouteRecordRaw } from "vue-router";
import { useAuthStore } from "@/stores/auth";

const routes: RouteRecordRaw[] = [
    { path: "/", redirect: "/dashboard" },
    { path: "/setup", component: () => import("@/views/SetupView.vue"), meta: { public: true } },
    { path: "/unlock", component: () => import("@/views/UnlockView.vue"), meta: { public: true } },
    {
        path: "/",
        component: () => import("@/layouts/AppLayout.vue"),
        children: [
            { path: "dashboard", component: () => import("@/views/DashboardView.vue") },
            { path: "portfolio", component: () => import("@/views/PortfolioView.vue") },
            { path: "goals", component: () => import("@/views/GoalsView.vue") },
            { path: "transactions", component: () => import("@/views/TransactionsView.vue") },
            { path: "liabilities", component: () => import("@/views/LiabilitiesView.vue") },
            { path: "income-expenses", component: () => import("@/views/IncomeExpensesView.vue") },
            { path: "data-sources", component: () => import("@/views/DataSourcesView.vue") },
            { path: "reports", component: () => import("@/views/ReportsView.vue") },
            { path: "settings", component: () => import("@/views/SettingsView.vue") },
        ],
    },
];

const router = createRouter({
    history: createWebHashHistory(),
    routes,
});

router.beforeEach(async (to) => {
    if (to.meta.public) return true;

    const auth = useAuthStore();
    await auth.init();

    if (!auth.isPasswordSet) {
        return "/setup";
    }
    if (!auth.isUnlocked) {
        return "/unlock";
    }
    return true;
});

export default router;
