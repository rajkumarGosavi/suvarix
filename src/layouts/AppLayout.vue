<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from "vue";
import { useRouter, useRoute } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useUiStore } from "@/stores/ui";
import { useAuthStore } from "@/stores/auth";
import { useAnalytics } from "@/composables/useAnalytics";
import { useToast } from "primevue/usetoast";
import { useRemindersStore } from "@/stores/reminders";
import { useInsightsStore } from "@/stores/insights";
import { useNotifications } from "@/composables/useNotifications";
import { useGoalCheck } from "@/composables/useGoalCheck";
import { useMaturityCheck } from "@/composables/useMaturityCheck";
import { APP_NAME } from "@/constants";

const ui = useUiStore();
const auth = useAuthStore();
const router = useRouter();
const route = useRoute();
const { track } = useAnalytics();
const toast = useToast();
const remindersStore = useRemindersStore();
const insights = useInsightsStore();
const { nativeNotify } = useNotifications();
const { checkGoals } = useGoalCheck();
const { checkMaturity } = useMaturityCheck();

// ─── Auto-sync notifications ───────────────────────────────────
// Only fires when a background tick actually pulled newer data from
// another device — a routine push-only tick with no diff stays silent.
let unlistenAutoSync: UnlistenFn | null = null;

// ─── Sync version-mismatch banner ──────────────────────────────
// Persistent (not a one-off Toast, see feedback_toast_pattern) — a peer wrote
// a newer `.svbak` format than this app build reads, so auto-sync is paused
// until the app updates. Local (non-sync) data is unaffected either way.
// Session-local dismiss only: reappears next launch until actually fixed.
const syncBlocked = ref(false);
const syncBannerDismissed = ref(false);

async function checkSyncBlockStatus() {
    syncBlocked.value = await invoke<boolean>("get_sync_block_status").catch(() => false);
}

async function lock() {
    await auth.lock();
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
        nativeNotify(`${APP_NAME} — Recurring due`, detail);
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
        nativeNotify(`${APP_NAME} — Bills due today`, detail);
    } else if (tomorrowBills.length > 0) {
        const names = tomorrowBills.map(r => r.name).join(", ");
        toast.add({ severity: "warn", summary: "Bills due tomorrow", detail: `Due tomorrow: ${names}`, life: 8000 });
    }

    // Check goal achievements against current net worth
    const nw = await invoke<{ totalAssets: number }>("get_net_worth").catch(() => null);
    if (nw) checkGoals(nw.totalAssets);

    // Check FD/bond maturity alerts (within 30 days or recently matured)
    await checkMaturity(30);
}

onMounted(async () => {
    track("app_opened");
    await loadLockSetting();
    checkReminders();
    checkSyncBlockStatus();
    insights.fetch(); // nav badge count (feed itself refreshes on Dashboard)
    ACTIVITY_EVENTS.forEach(e => window.addEventListener(e, onActivity, { passive: true }));
    lockTimer = setInterval(() => {
        if (auth.checkAutoLock(autoLockMs)) {
            router.push("/unlock");
        }
        checkSyncBlockStatus();
    }, 60_000);
    unlistenAutoSync = await listen("auto-sync-imported", () => {
        toast.add({
            severity: "info",
            summary: "Synced from another device",
            detail: "New data was pulled in from your sync folder. Restart the app to see it.",
            life: 10000,
        });
        checkSyncBlockStatus();
    });
});

onUnmounted(() => {
    ACTIVITY_EVENTS.forEach(e => window.removeEventListener(e, onActivity));
    if (lockTimer) clearInterval(lockTimer);
    if (unlistenAutoSync) unlistenAutoSync();
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

const drawerOpen = ref(false);
watch(route, () => { drawerOpen.value = false; });
</script>

<template>
    <div class="app-shell">
        <!-- Mobile top bar (hidden on desktop) -->
        <header class="mobile-topbar">
            <Button icon="pi pi-bars" text size="small" @click="drawerOpen = true" aria-label="Open menu" />
            <Logo :show-wordmark="true" :size="24" />
            <Button icon="pi pi-lock" text size="small" @click="lock" aria-label="Lock app" class="ml-auto" />
        </header>

        <!-- Drawer backdrop (mobile only) -->
        <div v-if="drawerOpen" class="drawer-backdrop" @click="drawerOpen = false" />

        <nav class="sidebar" :class="{ collapsed: ui.sidebarCollapsed, 'drawer-open': drawerOpen }">
            <div class="sidebar-brand">
                <Logo :show-wordmark="!ui.sidebarCollapsed" :size="28" />
                <!-- Desktop: collapse toggle -->
                <Button
                    :icon="ui.sidebarCollapsed ? 'pi pi-bars' : 'pi pi-times'"
                    text
                    size="small"
                    @click="ui.toggleSidebar()"
                    class="toggle-btn desktop-only"
                    aria-label="Toggle sidebar"
                />
                <!-- Mobile: close drawer -->
                <Button
                    icon="pi pi-times"
                    text
                    size="small"
                    @click="drawerOpen = false"
                    class="toggle-btn mobile-close-btn"
                    aria-label="Close menu"
                />
            </div>

            <div class="nav-list">
                <div v-for="item in navItems" :key="item.path" class="nav-item-wrap">
                    <Button
                        :icon="item.icon"
                        :label="ui.sidebarCollapsed ? undefined : item.label"
                        text
                        class="nav-btn"
                        :class="{ 'nav-btn--active': isActive(item.path) }"
                        @click="router.push(item.path)"
                        :aria-label="item.label"
                        v-tooltip.right="ui.sidebarCollapsed ? item.label : undefined"
                    />
                    <span
                        v-if="item.path === '/dashboard' && insights.urgentCount > 0"
                        class="nav-badge"
                        :aria-label="`${insights.urgentCount} insights to act on`"
                    >{{ insights.urgentCount }}</span>
                </div>
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

        <div class="main-content-wrap">
            <div v-if="syncBlocked && !syncBannerDismissed" class="sync-block-banner">
                <i class="pi pi-exclamation-triangle" />
                <span>A newer app version wrote your synced data — update {{ APP_NAME }} to resume auto-sync. Your local data isn't affected.</span>
                <Button icon="pi pi-times" text size="small" @click="syncBannerDismissed = true" aria-label="Dismiss" />
            </div>
            <main class="main-content">
                <RouterView />
            </main>
        </div>
    </div>

    <Toast position="bottom-right" />
</template>

<style scoped>
.app-shell {
    display: flex;
    height: 100dvh;
    overflow: hidden;
}

.sidebar {
    display: flex;
    flex-direction: column;
    width: 220px;
    min-width: 220px;
    transition: width 0.2s ease, min-width 0.2s ease;
    overflow: hidden;
    background: var(--p-content-background);
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

.nav-item-wrap {
    position: relative;
}

.nav-btn {
    width: 100%;
    justify-content: flex-start;
    border-radius: 8px !important;
    font-size: 0.875rem;
    white-space: nowrap;
}

/* Urgent-insight count on the Dashboard nav item. */
.nav-badge {
    position: absolute;
    top: 50%;
    right: 0.5rem;
    transform: translateY(-50%);
    min-width: 1.1rem;
    height: 1.1rem;
    padding: 0 0.3rem;
    border-radius: 999px;
    background: var(--p-red-500, #ef4444);
    color: #fff;
    font-size: 0.7rem;
    font-weight: 700;
    line-height: 1.1rem;
    text-align: center;
    pointer-events: none;
}

/* Collapsed rail: nudge the badge to the top-right corner of the icon. */
.sidebar.collapsed .nav-badge {
    top: 0.35rem;
    right: 0.35rem;
    transform: none;
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

@media (min-width: 640px) {
    .sidebar.collapsed .sidebar-brand {
        flex-direction: column;
        gap: 0.4rem;
        padding: 0.75rem 0.5rem;
    }

    .sidebar.collapsed .toggle-btn {
        margin-left: 0;
    }

    .sidebar.collapsed .nav-btn,
    .sidebar.collapsed .lock-btn {
        justify-content: center;
    }
}

.main-content-wrap {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
}

.main-content {
    flex: 1;
    overflow-y: auto;
    padding: 1.75rem 2rem;
}

.sync-block-banner {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.6rem 1rem;
    background: color-mix(in srgb, var(--p-orange-400) 12%, var(--p-content-background));
    border-bottom: 1px solid color-mix(in srgb, var(--p-orange-400) 30%, var(--p-content-background));
    font-size: 0.85rem;
    flex-shrink: 0;
}

.sync-block-banner i {
    color: var(--p-orange-500);
}

.sync-block-banner span {
    flex: 1;
}

/* ── Mobile (≤639px): drawer navigation ───────────────────── */

.mobile-topbar {
    display: none;
}

.drawer-backdrop {
    display: none;
}

/* Mobile close btn hidden on desktop; desktop toggle hidden on mobile */
.mobile-close-btn {
    display: none;
}

@media (max-width: 639px) {
    .app-shell {
        flex-direction: column;
    }

    .mobile-topbar {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        padding: env(safe-area-inset-top, 0px) 0.75rem 0;
        height: calc(52px + env(safe-area-inset-top, 0px));
        box-sizing: border-box;
        flex-shrink: 0;
        background: var(--p-content-background);
        border-bottom: 1px solid var(--p-content-border-color);
        position: sticky;
        top: 0;
        z-index: 50;
    }

    .ml-auto {
        margin-left: auto;
    }

    .drawer-backdrop {
        display: block;
        position: fixed;
        top: calc(52px + env(safe-area-inset-top, 0px));
        left: 0;
        right: 0;
        bottom: 0;
        background: rgba(0, 0, 0, 0.5);
        z-index: 199;
    }

    .sidebar {
        position: fixed;
        top: calc(52px + env(safe-area-inset-top, 0px));
        left: -240px;
        height: calc(100% - 52px - env(safe-area-inset-top, 0px));
        width: 240px !important;
        min-width: 240px !important;
        z-index: 200;
        transition: left 0.25s ease;
    }

    .sidebar.drawer-open {
        left: 0;
    }

    /* Swap toggle buttons: hide desktop collapse, show mobile close */
    .desktop-only {
        display: none;
    }

    .mobile-close-btn {
        display: flex;
    }

    .main-content {
        padding: 1rem 0.875rem;
    }
}
</style>
