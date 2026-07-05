import { createRouter, createWebHashHistory, type RouteRecordRaw } from "vue-router";
import { useAuthStore } from "@/stores/auth";
import { invoke } from "@tauri-apps/api/core";

const routes: RouteRecordRaw[] = [
    { path: "/", redirect: "/dashboard" },
    { path: "/setup", component: () => import("@/views/SetupView.vue"), meta: { public: true } },
    { path: "/unlock", component: () => import("@/views/UnlockView.vue"), meta: { public: true } },
    { path: "/eula", component: () => import("@/views/EulaGateView.vue"), meta: { public: true } },
    { path: "/onboarding", component: () => import("@/views/OnboardingView.vue"), meta: { public: true } },
    {
        path: "/",
        component: () => import("@/layouts/AppLayout.vue"),
        children: [
            { path: "dashboard", component: () => import("@/views/DashboardView.vue") },
            { path: "portfolio", component: () => import("@/views/PortfolioView.vue") },
            { path: "goals", component: () => import("@/views/GoalsView.vue") },
            { path: "transactions", component: () => import("@/views/TransactionsView.vue") },
            { path: "liabilities", component: () => import("@/views/LiabilitiesView.vue") },
            { path: "reminders", component: () => import("@/views/RemindersView.vue") },
            { path: "calendar", component: () => import("@/views/CalendarView.vue") },
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

let _navStart = 0;
router.beforeEach(() => { _navStart = performance.now(); return true; });
router.afterEach((to) => {
    const ms = Math.round(performance.now() - _navStart);
    invoke("track_perf", { metricName: `nav:${to.path}`, valueMs: ms }).catch(() => {});
    invoke("track_event", { name: "page_viewed", properties: JSON.stringify({ path: to.path }) }).catch(() => {});
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
    if (!auth.eulaCurrent) {
        return "/eula";
    }
    const needsOnboarding = import.meta.env.DEV
        ? !auth._onboardingSeen
        : !auth.onboardingComplete;
    if (needsOnboarding) {
        return "/onboarding";
    }
    return true;
});

export default router;
