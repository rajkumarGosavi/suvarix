<script setup lang="ts">
import { onMounted, onUnmounted } from "vue";
import { useRouter, useRoute } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { useUiStore } from "@/stores/ui";
import { useAuthStore } from "@/stores/auth";
import { useAnalytics } from "@/composables/useAnalytics";
import { useToast } from "primevue/usetoast";
import { useRemindersStore } from "@/stores/reminders";
import { useNotifications } from "@/composables/useNotifications";
import { useGoalCheck } from "@/composables/useGoalCheck";

const ui = useUiStore();
const auth = useAuthStore();
const router = useRouter();
const route = useRoute();
const { track } = useAnalytics();
const toast = useToast();
const remindersStore = useRemindersStore();
const { nativeNotify } = useNotifications();
const { checkGoals } = useGoalCheck();

function lock() {
    auth.lock();
    router.push("/unlock");
}

// ─── Auto-lock ────────────────────────────────────────────────
const ACTIVITY_EVENTS = ["mousemove", "keydown", "mousedown", "touchstart"] as const;
let lockTimer: ReturnType<typeof setInterval> | null = null;
let autoLockMs = 15 * 60 * 1000; // default 15 min

function onActivity() { auth.refreshActivity(); }

async function loadLockSetting() {
    try {
        const val = await invoke<string>("get_setting", { key: "auto_lock_minutes" });
        const mins = parseInt(val, 10);
        autoLockMs = mins > 0 ? mins * 60 * 1000 : 0;
    } catch { /* key not set yet — use default */ }
}

async function checkReminders() {
    // Check recurring transactions due today
    await remindersStore.loadDue();
    const dueCount = remindersStore.dueRecurring.length;
    if (dueCount > 0) {
        const detail = `${dueCount} recurring transaction${dueCount > 1 ? "s" : ""} due today. Open Reminders to apply.`;
        toast.add({ severity: "warn", summary: "Recurring transactions due", detail, life: 8000 });
        nativeNotify("FinFolio — Recurring due", detail);
    }

    // Check bills, loans, credit cards due today or tomorrow (days=1)
    const upcoming = await invoke<{ name: string; daysUntilDue: number }[]>(
        "get_upcoming_reminders", { days: 1 }
    ).catch(() => [] as { name: string; daysUntilDue: number }[]);
    const todayBills = upcoming.filter(r => r.daysUntilDue <= 0);
    const tomorrowBills = upcoming.filter(r => r.daysUntilDue === 1);

    if (todayBills.length > 0) {
        const names = todayBills.map(r => r.name).join(", ");
        const detail = `Due today: ${names}`;
        toast.add({ severity: "danger", summary: "Bills due today", detail, life: 10000 });
        nativeNotify("FinFolio — Bills due today", detail);
    } else if (tomorrowBills.length > 0) {
        const names = tomorrowBills.map(r => r.name).join(", ");
        toast.add({ severity: "warn", summary: "Bills due tomorrow", detail: names, life: 8000 });
    }

    // Check goal achievements against current net worth
    const nw = await invoke<{ totalAssets: number }>("get_net_worth").catch(() => null);
    if (nw) checkGoals(nw.totalAssets);
}

onMounted(async () => {
    track("app_opened");
    await loadLockSetting();
    checkReminders();
    ACTIVITY_EVENTS.forEach(e => window.addEventListener(e, onActivity, { passive: true }));
    lockTimer = setInterval(() => {
        if (auth.checkAutoLock(autoLockMs)) {
            router.push("/unlock");
        }
    }, 60_000);
});

onUnmounted(() => {
    ACTIVITY_EVENTS.forEach(e => window.removeEventListener(e, onActivity));
    if (lockTimer) clearInterval(lockTimer);
});

const navItems = [
    { path: "/dashboard",       icon: "pi pi-home",        label: "Dashboard" },
    { path: "/portfolio",       icon: "pi pi-briefcase",   label: "Portfolio" },
    { path: "/goals",           icon: "pi pi-flag",        label: "Goals" },
    { path: "/transactions",    icon: "pi pi-list",        label: "Transactions" },
    { path: "/liabilities",     icon: "pi pi-credit-card", label: "Liabilities" },
    { path: "/reminders",      icon: "pi pi-bell",        label: "Reminders" },
    { path: "/calendar",       icon: "pi pi-calendar",    label: "Calendar" },
    { path: "/income-expenses", icon: "pi pi-wallet",      label: "Income & Expenses" },
    { path: "/data-sources",    icon: "pi pi-database",    label: "Data Sources" },
    { path: "/reports",         icon: "pi pi-chart-bar",   label: "Reports" },
    { path: "/settings",        icon: "pi pi-cog",         label: "Settings" },
];

const isActive = (path: string) => route.path === path || route.path.startsWith(path + "/");
</script>

<template>
    <div class="app-shell">
        <nav class="sidebar" :class="{ collapsed: ui.sidebarCollapsed }">
            <div class="sidebar-brand">
                <span v-if="!ui.sidebarCollapsed" class="brand-name logo-brand">FinFolio</span>
                <Button
                    :icon="ui.sidebarCollapsed ? 'pi pi-bars' : 'pi pi-times'"
                    text
                    size="small"
                    @click="ui.toggleSidebar()"
                    class="toggle-btn"
                    aria-label="Toggle sidebar"
                />
            </div>

            <div class="nav-list">
                <Button
                    v-for="item in navItems"
                    :key="item.path"
                    :icon="item.icon"
                    :label="ui.sidebarCollapsed ? undefined : item.label"
                    text
                    class="nav-btn"
                    :class="{ 'nav-btn--active': isActive(item.path) }"
                    @click="router.push(item.path)"
                    :aria-label="item.label"
                    v-tooltip.right="ui.sidebarCollapsed ? item.label : undefined"
                />
            </div>

            <div class="sidebar-footer">
                <Divider />
                <Button
                    icon="pi pi-lock"
                    :label="ui.sidebarCollapsed ? undefined : 'Lock App'"
                    text
                    class="nav-btn lock-btn"
                    @click="lock"
                    v-tooltip.right="ui.sidebarCollapsed ? 'Lock App' : undefined"
                />
            </div>
        </nav>

        <main class="main-content">
            <RouterView />
        </main>
    </div>

    <Toast position="bottom-right" />
    <ConfirmDialog />
</template>

<style scoped>
.app-shell {
    display: flex;
    height: 100vh;
    overflow: hidden;
}

.sidebar {
    display: flex;
    flex-direction: column;
    width: 220px;
    min-width: 220px;
    transition: width 0.2s ease, min-width 0.2s ease;
    overflow: hidden;
    background: var(--p-surface-card);
    border-right: 1px solid var(--p-content-border-color);
}

.sidebar.collapsed {
    width: 62px;
    min-width: 62px;
}

.sidebar-brand {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.75rem 0.6rem 0.75rem 1rem;
    min-height: 56px;
    border-bottom: 1px solid var(--p-content-border-color);
}

.brand-name {
    font-size: 1.15rem;
    font-weight: 800;
    white-space: nowrap;
}

.toggle-btn {
    flex-shrink: 0;
    margin-left: auto;
}

.nav-list {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    padding: 0.75rem 0.4rem;
    overflow-y: auto;
}

.nav-btn {
    width: 100%;
    justify-content: flex-start;
    border-radius: 8px !important;
    font-size: 0.875rem;
    white-space: nowrap;
}

.nav-btn--active {
    font-weight: 600;
    background: var(--p-primary-50) !important;
    color: var(--p-primary-color) !important;
    border-left: 3px solid var(--p-primary-color) !important;
    padding-left: calc(0.75rem - 3px) !important;
}

.sidebar-footer {
    padding: 0 0.4rem 0.5rem;
}

.lock-btn {
    width: 100%;
    justify-content: flex-start;
}

.main-content {
    flex: 1;
    overflow-y: auto;
    padding: 1.75rem 2rem;
}
</style>
