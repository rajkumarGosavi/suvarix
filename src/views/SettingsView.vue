<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { save, open } from "@tauri-apps/plugin-dialog";
import { isEnabled as isAutostartEnabled, enable as enableAutostart, disable as disableAutostart } from "@tauri-apps/plugin-autostart";
import { useConfirm } from "primevue/useconfirm";
import { useToast } from "primevue/usetoast";
import { useUiStore } from "@/stores/ui";
import type { Theme } from "@/stores/ui";
import { usePortfolioStore } from "@/stores/portfolio";
import { useBudgetStore } from "@/stores/budget";
import { useRemindersStore } from "@/stores/reminders";
import { APP_NAME } from "@/constants";
import { useAnalytics } from "@/composables/useAnalytics";
import { friendlyError } from "@/utils/errorMessage";

// Zerodha/Upstox OAuth uses this same check elsewhere (DataSourcesView) —
// tauri-plugin-dialog has no directory picker on Android, so auto-sync's
// folder picker goes through a native command there instead (see below).
const isAndroid = /android/i.test(navigator.userAgent);

// ─── Analytics types ─────────────────────────────────────────
interface EventStat   { eventName: string; count: number; lastSeen: string; }
interface ErrorEntry  { id: number; errorType: string; message: string; createdAt: string; }
interface PerfStat    { metricName: string; avgMs: number; count: number; }
interface AnalyticsExport { exportedAt: string; appVersion: string; events: EventStat[]; errors: ErrorEntry[]; perf: PerfStat[]; }

const ui = useUiStore();
const portfolio = usePortfolioStore();
const budget = useBudgetStore();
const reminders = useRemindersStore();
const { trackError } = useAnalytics();

// ─── Appearance ──────────────────────────────────────────────
const THEME_OPTIONS: { label: string; value: Theme; icon: string }[] = [
    { label: "Light",  value: "light",  icon: "pi pi-sun" },
    { label: "System", value: "system", icon: "pi pi-desktop" },
    { label: "Dark",   value: "dark",   icon: "pi pi-moon" },
];

const confirm = useConfirm();
const toast = useToast();

// ─── Auto-lock ───────────────────────────────────────────────
const LOCK_OPTIONS = [
    { label: "5 minutes",  value: "5" },
    { label: "15 minutes", value: "15" },
    { label: "30 minutes", value: "30" },
    { label: "1 hour",     value: "60" },
    { label: "Disabled",   value: "0" },
];
const selectedLock = ref("15");
const lockSaving = ref(false);

async function loadLockSetting() {
    try {
        const val = await invoke<string>("get_setting", { key: "auto_lock_minutes" });
        if (LOCK_OPTIONS.some(o => o.value === val)) selectedLock.value = val;
    } catch { /* not set yet */ }
}

async function saveLockSetting() {
    lockSaving.value = true;
    try {
        await invoke("set_setting", { key: "auto_lock_minutes", value: selectedLock.value });
        toast.add({ severity: "success", summary: "Saved", detail: "Auto-lock setting updated.", life: 2500 });
    } finally {
        lockSaving.value = false;
    }
}

// ─── Emergency-fund target ───────────────────────────────────
const emergencyMonths = ref<number>(6);
const emergencySaving = ref(false);

async function loadEmergencyTarget() {
    try {
        const val = await invoke<string>("get_setting", { key: "emergency_fund_target_months" });
        const n = parseInt(val, 10);
        if (Number.isFinite(n) && n > 0) emergencyMonths.value = n;
    } catch { /* not set yet — default 6 */ }
}

async function saveEmergencyTarget() {
    let n = Math.round(emergencyMonths.value);
    if (!Number.isFinite(n) || n < 1) n = 1;
    if (n > 24) n = 24;
    emergencyMonths.value = n;
    emergencySaving.value = true;
    try {
        await invoke("set_setting", { key: "emergency_fund_target_months", value: String(n) });
        toast.add({ severity: "success", summary: "Saved", detail: "Emergency-fund target updated. Health score will use it on next refresh.", life: 2800 });
    } finally {
        emergencySaving.value = false;
    }
}

// ─── Launch at login ─────────────────────────────────────────
const autostartEnabled = ref(false);
const autostartLoading = ref(false);

async function toggleAutostart(val: boolean) {
    autostartLoading.value = true;
    try {
        if (val) await enableAutostart(); else await disableAutostart();
        autostartEnabled.value = val;
    } catch (e: any) {
        toast.add({ severity: "error", summary: "Failed", detail: friendlyError(e, "Couldn't update launch-at-login setting. Try again."), life: 4000 });
    } finally {
        autostartLoading.value = false;
    }
}

// ─── Security ────────────────────────────────────────────────
const currentPw = ref("");
const newPw = ref("");
const confirmPw = ref("");
const pwError = ref("");
const pwSuccess = ref("");
const pwLoading = ref(false);

async function changePw() {
    pwError.value = "";
    pwSuccess.value = "";
    if (newPw.value.length < 8) { pwError.value = "New password must be at least 8 characters."; return; }
    if (newPw.value !== confirmPw.value) { pwError.value = "Passwords do not match."; return; }
    pwLoading.value = true;
    try {
        await invoke("change_master_password", { currentPassword: currentPw.value, newPassword: newPw.value });
        pwSuccess.value = "Password changed successfully.";
        currentPw.value = ""; newPw.value = ""; confirmPw.value = "";
    } catch (e: any) {
        pwError.value = friendlyError(e, "Failed to change password.");
    } finally {
        pwLoading.value = false;
    }
}

// ─── Data Management ─────────────────────────────────────────
const backupLoading = ref(false);
const restoreLoading = ref(false);
const wipeConfirmText = ref("");
const wipeDialogVisible = ref(false);
const wipeLoading = ref(false);

async function doBackup() {
    backupLoading.value = true;
    try {
        const dest = await save({
            defaultPath: `${APP_NAME.toLowerCase()}-backup-${new Date().toISOString().slice(0, 10)}.db`,
            filters: [{ name: "SQLite Database", extensions: ["db"] }],
        });
        if (!dest) return;
        await invoke("backup_database", { destPath: dest });
        toast.add({ severity: "success", summary: "Backup saved", detail: dest, life: 4000 });
    } catch (e: any) {
        toast.add({ severity: "error", summary: "Backup failed", detail: friendlyError(e, "Couldn't save the backup. Try again."), life: 5000 });
    } finally {
        backupLoading.value = false;
    }
}

async function doRestore() {
    confirm.require({
        message: "Restoring from a backup will overwrite ALL current data. Continue?",
        header: "Restore from Backup",
        icon: "pi pi-exclamation-triangle",
        rejectProps: { label: "Cancel", outlined: true },
        acceptProps: { label: "Choose File & Restore" },
        accept: async () => {
            restoreLoading.value = true;
            try {
                const src = await open({
                    multiple: false,
                    filters: [{ name: "SQLite Database", extensions: ["db"] }],
                });
                if (!src) return;
                await invoke("restore_database", { srcPath: src as string });
                toast.add({ severity: "success", summary: "Restore complete", detail: "Restart the app to see restored data.", life: 6000 });
            } catch (e: any) {
                toast.add({ severity: "error", summary: "Restore failed", detail: friendlyError(e, "Couldn't restore the backup. Try again."), life: 5000 });
            } finally {
                restoreLoading.value = false;
            }
        },
    });
}

function openWipeDialog() {
    wipeConfirmText.value = "";
    wipeDialogVisible.value = true;
}

async function doWipe() {
    if (wipeConfirmText.value !== "DELETE") return;
    wipeLoading.value = true;
    try {
        await invoke("wipe_all_data");
        wipeDialogVisible.value = false;
        // DB is clean now, but every Pinia store still holds pre-wipe data in
        // memory (gamification XP/badges, portfolio holdings, transactions, …).
        // Hard-reload the app so all screens re-fetch from the wiped DB and show
        // empty state — cheaper and more reliable than refetching each store.
        window.location.reload();
        return;
    } catch (e: any) {
        toast.add({ severity: "error", summary: "Wipe failed", detail: friendlyError(e, "Couldn't delete the data. Try again."), life: 5000 });
    } finally {
        wipeLoading.value = false;
    }
}

// ─── Diagnostics ─────────────────────────────────────────────
const eventStats  = ref<EventStat[]>([]);
const errorLog    = ref<ErrorEntry[]>([]);
const perfStats   = ref<PerfStat[]>([]);
const diagLoading = ref(false);
const exportingDiag = ref(false);

async function loadDiagnostics() {
    diagLoading.value = true;
    try {
        [eventStats.value, errorLog.value, perfStats.value] = await Promise.all([
            invoke<EventStat[]>("get_event_stats"),
            invoke<ErrorEntry[]>("get_error_log"),
            invoke<PerfStat[]>("get_perf_stats"),
        ]);
    } finally {
        diagLoading.value = false;
    }
}

async function exportDiag() {
    exportingDiag.value = true;
    try {
        const data = await invoke<AnalyticsExport>("export_analytics");
        const json = JSON.stringify(data, null, 2);
        const dest = await save({
            defaultPath: `${APP_NAME.toLowerCase()}-diagnostics-${new Date().toISOString().slice(0, 10)}.json`,
            filters: [{ name: "JSON", extensions: ["json"] }],
        });
        if (!dest) return;
        await invoke("write_csv", { path: dest, content: json });
        toast.add({ severity: "success", summary: "Exported", detail: dest, life: 4000 });
    } catch (e: any) {
        toast.add({ severity: "error", summary: "Export failed", detail: friendlyError(e, "Couldn't export diagnostics. Try again."), life: 5000 });
    } finally {
        exportingDiag.value = false;
    }
}

function clearDiag() {
    confirm.require({
        message: "Delete all recorded events, errors, and performance data?",
        header: "Clear Diagnostics",
        icon: "pi pi-exclamation-triangle",
        rejectProps: { label: "Cancel", severity: "secondary", outlined: true },
        acceptProps: { label: "Clear", severity: "danger" },
        accept: async () => {
            await invoke("clear_analytics");
            eventStats.value = [];
            errorLog.value = [];
            perfStats.value = [];
            toast.add({ severity: "success", summary: "Diagnostics cleared", life: 2500 });
        },
    });
}

// ─── Sync backup ─────────────────────────────────────────────
const syncPwVisible = ref(false);
const syncPassword = ref("");
const syncMode = ref<"export" | "import">("export");
const syncPath = ref("");
const syncLoading = ref(false);

async function startSyncExport() {
    const dest = await save({
        defaultPath: `${APP_NAME.toLowerCase()}-sync-${new Date().toISOString().slice(0, 10)}.svbak`,
        filters: [{ name: `${APP_NAME} Sync Backup`, extensions: ["svbak"] }],
    });
    if (!dest) return;
    syncPath.value = dest as string;
    syncMode.value = "export";
    syncPassword.value = "";
    syncPwVisible.value = true;
}

async function startSyncImport() {
    confirm.require({
        message: "This will replace ALL financial data on this device with the backup's data. Your master password is NOT changed. Broker API keys will also be synced.",
        header: "Import Sync Backup",
        icon: "pi pi-exclamation-triangle",
        rejectProps: { label: "Cancel", outlined: true },
        acceptProps: { label: "Choose File & Import", severity: "danger" },
        accept: async () => {
            const src = await open({
                multiple: false,
                filters: [{ name: `${APP_NAME} Sync Backup`, extensions: ["svbak"] }],
            });
            if (!src) return;
            syncPath.value = src as string;
            syncMode.value = "import";
            syncPassword.value = "";
            syncPwVisible.value = true;
        },
    });
}

async function confirmSync() {
    if (!syncPassword.value || syncLoading.value) return;
    syncPwVisible.value = false;
    syncLoading.value = true;
    try {
        if (syncMode.value === "export") {
            const r = await invoke<{ rowsExported: number }>(
                "export_sync_backup",
                { destPath: syncPath.value, password: syncPassword.value },
            );
            toast.add({ severity: "success", summary: "Sync backup exported", detail: `${r.rowsExported} records saved.`, life: 5000 });
        } else {
            const r = await invoke<{ rowsImported: number; tablesImported: number }>(
                "import_sync_backup",
                { srcPath: syncPath.value, password: syncPassword.value },
            );
            toast.add({ severity: "success", summary: "Import complete", detail: `${r.rowsImported} records restored. Restart the app to refresh all views.`, life: 8000 });
        }
    } catch (e: any) {
        const detail = friendlyError(e, syncMode.value === "export" ? "Couldn't export the sync backup. Try again." : "Couldn't import the sync backup. Try again.");
        toast.add({ severity: "error", summary: syncMode.value === "export" ? "Export failed" : "Import failed", detail, life: 6000 });
        trackError(syncMode.value === "export" ? "sync_export_failed" : "sync_import_failed", String(e?.message?.message ?? e?.message ?? e));
    } finally {
        syncLoading.value = false;
        syncPassword.value = "";
    }
}

// ─── Auto Sync ───────────────────────────────────────────────
const AUTO_SYNC_INTERVAL_OPTIONS = [
    { label: "15 minutes", value: "15" },
    { label: "30 minutes", value: "30" },
    { label: "1 hour",     value: "60" },
];
const autoSyncEnabled = ref(false);
const autoSyncFolder = ref("");
const autoSyncInterval = ref("30");
const autoSyncHasPassword = ref(false);
const autoSyncPwVisible = ref(false);
const autoSyncPwInput = ref("");
const autoSyncSettingSaving = ref(false);
const autoSyncNowLoading = ref(false);
const lastSyncAt = ref("");
// The `exported_at` this device actually decrypted out of the shared file on
// its last import attempt — distinct from lastSyncAt (this device's own last
// *export* time). If this stays behind what the other device's own "Last
// synced" shows, this device is reading a stale/wrong copy of the file, not
// failing to merge fresh content it already has — a direct way to tell those
// two failure modes apart without needing device logs.
const lastRemoteExportedAtSeen = ref("");

async function loadAutoSyncSettings() {
    try { autoSyncFolder.value = await invoke<string>("get_setting", { key: "sync_folder_path" }); } catch { /* not set yet */ }
    try { autoSyncEnabled.value = (await invoke<string>("get_setting", { key: "auto_sync_enabled" })) === "true"; } catch { /* not set yet */ }
    try {
        const v = await invoke<string>("get_setting", { key: "auto_sync_interval_minutes" });
        if (AUTO_SYNC_INTERVAL_OPTIONS.some(o => o.value === v)) autoSyncInterval.value = v;
    } catch { /* not set yet */ }
    try { lastSyncAt.value = await invoke<string>("get_setting", { key: "last_sync_exported_at" }); } catch { /* not set yet */ }
    try { lastRemoteExportedAtSeen.value = await invoke<string>("get_setting", { key: "last_remote_exported_at_seen" }); } catch { /* not set yet */ }
    try { autoSyncHasPassword.value = await invoke<boolean>("has_sync_password"); } catch { /* non-critical */ }
}

// Android stores a JSON-encoded SAF tree URI here, not a human path — show
// a friendly label instead of the raw JSON blob.
const autoSyncFolderDisplay = computed(() => {
    if (!autoSyncFolder.value) return "Not set";
    return isAndroid ? "Folder selected" : autoSyncFolder.value;
});

async function chooseAutoSyncFolder() {
    // plugin-dialog's directory picker is desktop-only; Android goes through
    // a native SAF picker command that also persists the grant across app
    // restarts (needed for the background scheduler to keep working).
    const dir = isAndroid
        ? await invoke<string | null>("pick_sync_folder_android")
        : await open({ directory: true });
    if (!dir) return;
    autoSyncFolder.value = dir as string;
    await invoke("set_setting", { key: "sync_folder_path", value: autoSyncFolder.value });
}

async function toggleAutoSync(val: boolean) {
    if (val && (!autoSyncFolder.value || !autoSyncHasPassword.value)) {
        toast.add({ severity: "warn", summary: "Setup needed", detail: "Choose a sync folder and set a sync password first.", life: 4000 });
        return;
    }
    autoSyncSettingSaving.value = true;
    try {
        await invoke("set_setting", { key: "auto_sync_enabled", value: val ? "true" : "false" });
        autoSyncEnabled.value = val;
    } finally {
        autoSyncSettingSaving.value = false;
    }
}

async function saveAutoSyncInterval() {
    autoSyncSettingSaving.value = true;
    try {
        await invoke("set_setting", { key: "auto_sync_interval_minutes", value: autoSyncInterval.value });
        toast.add({ severity: "success", summary: "Saved", detail: "Auto-sync interval updated.", life: 2500 });
    } finally {
        autoSyncSettingSaving.value = false;
    }
}

async function saveAutoSyncPassword() {
    if (!autoSyncPwInput.value) return;
    try {
        await invoke("set_sync_password", { password: autoSyncPwInput.value });
        autoSyncHasPassword.value = true;
        autoSyncPwVisible.value = false;
        toast.add({ severity: "success", summary: "Sync password set", life: 3000 });
    } catch (e: any) {
        toast.add({ severity: "error", summary: "Failed", detail: friendlyError(e, "Couldn't set the sync password. Try again."), life: 4000 });
    } finally {
        autoSyncPwInput.value = "";
    }
}

async function syncNow() {
    if (!autoSyncFolder.value || !autoSyncHasPassword.value) {
        toast.add({ severity: "warn", summary: "Setup needed", detail: "Choose a sync folder and set a sync password first.", life: 4000 });
        return;
    }
    autoSyncNowLoading.value = true;
    try {
        const r = await invoke<{ ran: boolean; imported: boolean; exportedAt: string | null }>("sync_now");
        if (!r.ran) {
            toast.add({ severity: "warn", summary: "Sync skipped", detail: "Turn on Auto Sync above, then try again.", life: 4000 });
            return;
        }
        if (r.exportedAt) lastSyncAt.value = r.exportedAt;
        try { lastRemoteExportedAtSeen.value = await invoke<string>("get_setting", { key: "last_remote_exported_at_seen" }); } catch { /* not set yet */ }
        toast.add({
            severity: "success",
            summary: "Synced",
            detail: r.imported ? "Pulled newer data from the sync folder and pushed local changes." : "Pushed local changes to the sync folder.",
            life: 4000,
        });
    } catch (e: any) {
        toast.add({ severity: "error", summary: "Sync failed", detail: friendlyError(e, "Couldn't sync right now. Try again."), life: 5000 });
    } finally {
        autoSyncNowLoading.value = false;
    }
}

// ─── Developer / Dummy Data ──────────────────────────────────
const isDevBuild = ref(false);
const dummyDataEnabled = ref(false);
const dummyDataLoading = ref(false);

async function toggleDummyData(val: boolean) {
    dummyDataLoading.value = true;
    try {
        if (val) {
            await invoke("seed_dummy_data");
            dummyDataEnabled.value = true;
            await Promise.all([
                portfolio.fetchAll(),
                budget.fetchAll(),
                reminders.fetchBills(),
                reminders.fetchRecurring(),
                reminders.loadUpcoming(30),
            ]);
            toast.add({ severity: "success", summary: "Dummy data loaded", detail: "Portfolio, budgets and reminders refreshed.", life: 4000 });
        } else {
            await invoke("clear_dummy_data");
            dummyDataEnabled.value = false;
            await Promise.all([
                portfolio.fetchAll(),
                budget.fetchAll(),
                reminders.fetchBills(),
                reminders.fetchRecurring(),
                reminders.loadUpcoming(30),
            ]);
            toast.add({ severity: "info", summary: "Dummy data cleared", detail: "All demo records removed.", life: 3000 });
        }
    } catch (e: any) {
        toast.add({ severity: "error", summary: "Failed", detail: friendlyError(e, val ? "Couldn't load demo data. Try again." : "Couldn't clear demo data. Try again."), life: 4000 });
    } finally {
        dummyDataLoading.value = false;
    }
}

// ─── About ───────────────────────────────────────────────────
const appDataDir = ref("");

onMounted(async () => {
    await loadLockSetting();
    await loadEmergencyTarget();
    await loadAutoSyncSettings();
    await loadDiagnostics();
    try {
        autostartEnabled.value = await isAutostartEnabled();
    } catch { /* non-critical */ }
    try {
        appDataDir.value = await invoke<string>("get_app_data_dir");
    } catch { /* non-critical */ }
    try {
        isDevBuild.value = await invoke<boolean>("is_dev_build");
        if (isDevBuild.value) {
            dummyDataEnabled.value = await invoke<boolean>("is_dummy_data_seeded");
        }
    } catch { /* non-critical */ }
});

const appVersion = ref("");
getVersion().then(v => appVersion.value = v);
</script>

<template>
    <div class="settings-view">
        <h1 class="page-title">Settings</h1>

        <!-- Security -->
        <div class="section-card">
            <h2>Security</h2>
            <form @submit.prevent="changePw" class="pw-form">
                <div class="field">
                    <label>Current Password</label>
                    <Password v-model="currentPw" :feedback="false" toggleMask fluid placeholder="Current password" />
                </div>
                <div class="field">
                    <label>New Password</label>
                    <Password v-model="newPw" :feedback="true" toggleMask fluid placeholder="New password (min 8 chars)" />
                </div>
                <div class="field">
                    <label>Confirm New Password</label>
                    <Password v-model="confirmPw" :feedback="false" toggleMask fluid placeholder="Confirm new password" />
                </div>
                <Message v-if="pwError" severity="error">{{ pwError }}</Message>
                <Message v-if="pwSuccess" severity="success">{{ pwSuccess }}</Message>
                <div>
                    <Button type="submit" label="Change Password" :loading="pwLoading" />
                </div>
            </form>

            <Divider />

            <div class="lock-row">
                <div class="data-row-info">
                    <span class="data-row-title">Auto-lock</span>
                    <span class="data-row-desc">Lock the app after this period of inactivity.</span>
                </div>
                <div class="lock-control">
                    <Select
                        v-model="selectedLock"
                        :options="LOCK_OPTIONS"
                        optionLabel="label"
                        optionValue="value"
                        style="width:150px"
                    />
                    <Button label="Save" size="small" :loading="lockSaving" @click="saveLockSetting" />
                </div>
            </div>

            <Divider />

            <div class="data-row">
                <div class="data-row-info">
                    <span class="data-row-title">Launch at login</span>
                    <span class="data-row-desc">Start {{ APP_NAME }} hidden in the system tray when you log in, so bill and maturity reminders keep notifying you even when the app isn't open.</span>
                </div>
                <ToggleSwitch
                    :modelValue="autostartEnabled"
                    :disabled="autostartLoading"
                    @update:modelValue="toggleAutostart"
                />
            </div>
        </div>

        <!-- Appearance -->
        <div class="section-card">
            <h2>Appearance</h2>
            <div class="theme-row">
                <div class="data-row-info">
                    <span class="data-row-title">Theme</span>
                    <span class="data-row-desc">Choose Light, Dark, or follow your system setting.</span>
                </div>
                <SelectButton
                    :modelValue="ui.theme"
                    @update:modelValue="ui.setTheme($event)"
                    :options="THEME_OPTIONS"
                    optionLabel="label"
                    optionValue="value"
                >
                    <template #option="{ option }">
                        <i :class="option.icon" style="margin-right:0.4rem" />{{ option.label }}
                    </template>
                </SelectButton>
            </div>
        </div>

        <!-- Financial Health -->
        <div class="section-card">
            <h2>Financial Health</h2>
            <div class="lock-row">
                <div class="data-row-info">
                    <span class="data-row-title">Emergency-fund target</span>
                    <span class="data-row-desc">Months of expenses to treat as a full emergency fund. Reaching this scores the Emergency Fund pillar 100/100 and earns the Safety Net badge. Advisors usually suggest 3–6.</span>
                </div>
                <div class="lock-control">
                    <InputNumber
                        v-model="emergencyMonths"
                        :min="1"
                        :max="24"
                        showButtons
                        suffix=" months"
                        style="width:150px"
                    />
                    <Button label="Save" size="small" :loading="emergencySaving" @click="saveEmergencyTarget" />
                </div>
            </div>
        </div>

        <!-- Data Management -->
        <div class="section-card">
            <h2>Data Management</h2>

            <div class="data-row">
                <div class="data-row-info">
                    <span class="data-row-title">Backup Database</span>
                    <span class="data-row-desc">Save a copy of your entire database to a file.</span>
                </div>
                <Button
                    icon="pi pi-download"
                    label="Backup"
                    outlined
                    :loading="backupLoading"
                    @click="doBackup"
                />
            </div>

            <Divider />

            <div class="data-row">
                <div class="data-row-info">
                    <span class="data-row-title">Restore Database</span>
                    <span class="data-row-desc">Replace all current data with a previously saved backup.</span>
                </div>
                <Button
                    icon="pi pi-upload"
                    label="Restore"
                    outlined
                    :loading="restoreLoading"
                    @click="doRestore"
                />
            </div>

            <Divider />

            <div class="data-row">
                <div class="data-row-info">
                    <span class="data-row-title">Export Sync Backup</span>
                    <span class="data-row-desc">
                        Save an encrypted <code>.svbak</code> file to transfer to another device (USB, WhatsApp, etc.).
                        Protected by your master password.
                    </span>
                </div>
                <Button
                    icon="pi pi-file-export"
                    label="Export"
                    outlined
                    :loading="syncLoading && syncMode === 'export'"
                    @click="startSyncExport"
                />
            </div>

            <Divider />

            <div class="data-row">
                <div class="data-row-info">
                    <span class="data-row-title">Import Sync Backup</span>
                    <span class="data-row-desc">
                        Load a <code>.svbak</code> backup from another device. Replaces all current data.
                        Enter the source device's master password when prompted.
                    </span>
                </div>
                <Button
                    icon="pi pi-file-import"
                    label="Import"
                    outlined
                    :loading="syncLoading && syncMode === 'import'"
                    @click="startSyncImport"
                />
            </div>

            <Divider />

            <div class="data-row">
                <div class="data-row-info">
                    <span class="data-row-title">Auto Sync</span>
                    <span class="data-row-desc">
                        Automatically push/pull an encrypted <code>.svbak</code> snapshot to a folder you sync yourself
                        (Dropbox, Google Drive, OneDrive, etc.) while {{ APP_NAME }} is running. Edits made on two devices
                        at the same moment, before the cloud provider finishes syncing, may not both survive.
                    </span>
                </div>
                <ToggleSwitch
                    :modelValue="autoSyncEnabled"
                    :disabled="autoSyncSettingSaving"
                    @update:modelValue="toggleAutoSync"
                />
            </div>

            <div class="auto-sync-config">
                <div class="auto-sync-row">
                    <span class="auto-sync-label">Sync folder</span>
                    <div class="auto-sync-control">
                        <span class="auto-sync-path">{{ autoSyncFolderDisplay }}</span>
                        <Button label="Choose…" size="small" outlined @click="chooseAutoSyncFolder" />
                    </div>
                </div>
                <div class="auto-sync-row">
                    <span class="auto-sync-label">Sync password</span>
                    <div class="auto-sync-control">
                        <span class="auto-sync-path">{{ autoSyncHasPassword ? "Set" : "Not set" }}</span>
                        <Button label="Set…" size="small" outlined @click="autoSyncPwVisible = true" />
                    </div>
                </div>
                <div class="auto-sync-row">
                    <span class="auto-sync-label">Check interval</span>
                    <div class="auto-sync-control">
                        <Select
                            v-model="autoSyncInterval"
                            :options="AUTO_SYNC_INTERVAL_OPTIONS"
                            optionLabel="label"
                            optionValue="value"
                            style="width:150px"
                        />
                        <Button label="Save" size="small" :loading="autoSyncSettingSaving" @click="saveAutoSyncInterval" />
                    </div>
                </div>
                <div class="auto-sync-row">
                    <span class="auto-sync-label">Last synced</span>
                    <div class="auto-sync-control">
                        <span class="auto-sync-path">{{ lastSyncAt ? new Date(lastSyncAt).toLocaleString("en-IN") : "Never" }}</span>
                        <Button label="Sync Now" size="small" icon="pi pi-sync" :loading="autoSyncNowLoading" @click="syncNow" />
                    </div>
                </div>
                <div class="auto-sync-row">
                    <span class="auto-sync-label" v-tooltip.top="'The data timestamp this device actually read from the shared file last time it synced — compare this against the other device\'s own \'Last synced\' above. If this is stuck behind that, this device is reading a stale copy of the file, not failing to merge data it already has.'">
                        Last remote data seen
                    </span>
                    <div class="auto-sync-control">
                        <span class="auto-sync-path">{{ lastRemoteExportedAtSeen ? new Date(lastRemoteExportedAtSeen).toLocaleString("en-IN") : "Never" }}</span>
                    </div>
                </div>
            </div>

            <Divider />

            <div class="data-row">
                <div class="data-row-info">
                    <span class="data-row-title">Wipe All Data</span>
                    <span class="data-row-desc">
                        Permanently delete all portfolio, transaction, and liability records.
                        Your master password and app settings are not affected.
                    </span>
                </div>
                <Button
                    icon="pi pi-trash"
                    label="Wipe Data"
                    outlined
                    @click="openWipeDialog"
                />
            </div>
        </div>

        <!-- Diagnostics -->
        <div class="section-card">
            <div class="diag-header">
                <h2 style="margin:0">Diagnostics</h2>
                <div class="diag-actions">
                    <Button
                        icon="pi pi-download"
                        label="Export"
                        size="small"
                        outlined
                        :loading="exportingDiag"
                        @click="exportDiag"
                    />
                    <Button
                        icon="pi pi-trash"
                        label="Clear"
                        size="small"
                        outlined
                        severity="danger"
                        @click="clearDiag"
                    />
                </div>
            </div>
            <p class="diag-desc">
                Usage events, errors, and performance data stored locally.
                Export this file and share it with the developer for feedback.
            </p>

            <div v-if="diagLoading" class="diag-loading">
                <ProgressSpinner style="width:28px;height:28px" />
            </div>

            <template v-else>
                <!-- Feature usage -->
                <h3 class="diag-section-title">Feature Usage</h3>
                <div v-if="eventStats.length === 0" class="diag-empty">No events recorded yet.</div>
                <DataTable v-else :value="eventStats" size="small" class="diag-table">
                    <Column field="eventName" header="Event" />
                    <Column field="count" header="Count" style="width:80px;text-align:right" />
                    <Column field="lastSeen" header="Last seen" style="width:180px">
                        <template #body="{ data }">
                            {{ data.lastSeen ? new Date(data.lastSeen).toLocaleString("en-IN") : "—" }}
                        </template>
                    </Column>
                </DataTable>

                <Divider />

                <!-- Recent errors -->
                <h3 class="diag-section-title">Recent Errors</h3>
                <div v-if="errorLog.length === 0" class="diag-empty">No errors recorded.</div>
                <div v-else class="error-list">
                    <div v-for="err in errorLog.slice(0, 10)" :key="err.id" class="error-entry">
                        <span class="error-type">{{ err.errorType }}</span>
                        <span class="error-msg">{{ err.message }}</span>
                        <span class="error-time">{{ new Date(err.createdAt).toLocaleString("en-IN") }}</span>
                    </div>
                </div>

                <Divider />

                <!-- Performance -->
                <h3 class="diag-section-title">Performance</h3>
                <div v-if="perfStats.length === 0" class="diag-empty">No performance data yet.</div>
                <DataTable v-else :value="perfStats" size="small" class="diag-table">
                    <Column field="metricName" header="Screen / Event" />
                    <Column header="Avg (ms)" style="width:100px;text-align:right">
                        <template #body="{ data }">{{ Math.round(data.avgMs) }}</template>
                    </Column>
                    <Column field="count" header="Samples" style="width:90px;text-align:right" />
                </DataTable>
            </template>
        </div>

        <!-- Developer (dev builds only) -->
        <div v-if="isDevBuild" class="section-card dev-card">
            <h2>
                <i class="pi pi-code" style="margin-right:0.5rem;color:var(--p-orange-400)" />
                Developer
            </h2>
            <div class="data-row">
                <div class="data-row-info">
                    <span class="data-row-title">Dummy Data</span>
                    <span class="data-row-desc">
                        Populate all sections with realistic demo records (equity, MF, FD, gold, crypto, etc.).
                        Not available in release builds.
                    </span>
                </div>
                <ToggleSwitch
                    :modelValue="dummyDataEnabled"
                    :disabled="dummyDataLoading"
                    @update:modelValue="toggleDummyData"
                />
            </div>
        </div>

        <!-- About -->
        <div class="section-card">
            <h2>About</h2>
            <div class="about-grid">
                <span class="about-label">App</span>
                <span>{{ APP_NAME }} — Personal Finance Tracker</span>
                <span class="about-label">Version</span>
                <span>{{ appVersion }}</span>
                <span class="about-label">Data directory</span>
                <span class="about-path">{{ appDataDir || "—" }}</span>
                <span class="about-label">Privacy</span>
                <span>All data is stored locally on this device. Nothing is sent to the cloud.</span>
                <span class="about-label">macOS install</span>
                <span>
                    If macOS says <em>"Suvarix is damaged"</em>, open Terminal and run:<br />
                    <code>xattr -cr /Applications/Suvarix.app</code><br />
                    Then launch the app normally. This removes the quarantine flag added by macOS on downloaded files.
                </span>
            </div>
        </div>
    </div>

    <!-- Wipe confirmation dialog -->
    <Dialog v-model:visible="wipeDialogVisible" header="Wipe All Data" modal style="width:440px">
        <p>This will permanently delete all your financial data. This action cannot be undone.</p>
        <p>Type <strong>DELETE</strong> below to confirm:</p>
        <InputText
            v-model="wipeConfirmText"
            placeholder="Type DELETE to confirm"
            fluid
            class="mt-input"
            @keydown.enter="doWipe"
        />
        <div class="dialog-footer">
            <Button label="Cancel" outlined @click="wipeDialogVisible = false" />
            <Button
                label="Wipe All Data"
                :disabled="wipeConfirmText !== 'DELETE'"
                :loading="wipeLoading"
                @click="doWipe"
            />
        </div>
    </Dialog>

    <!-- Sync backup password dialog -->
    <Dialog
        v-model:visible="syncPwVisible"
        :header="syncMode === 'export' ? 'Enter Master Password to Encrypt' : 'Enter Source Device Password'"
        modal
        style="width: 380px"
    >
        <p class="sync-desc">
            {{ syncMode === 'export'
                ? 'The backup is encrypted with your master password. You will need this exact password to import it on another device.'
                : 'Enter the master password of the device that created this backup.' }}
        </p>
        <Password
            v-model="syncPassword"
            :feedback="false"
            toggleMask
            fluid
            placeholder="Master password"
            autofocus
            @keydown.enter="confirmSync"
        />
        <div class="dialog-footer">
            <Button label="Cancel" outlined @click="syncPwVisible = false; syncPassword = ''" />
            <Button
                :label="syncMode === 'export' ? 'Encrypt & Save' : 'Decrypt & Import'"
                :disabled="!syncPassword"
                :loading="syncLoading"
                @click="confirmSync"
            />
        </div>
    </Dialog>

    <!-- Auto-sync password dialog -->
    <Dialog v-model:visible="autoSyncPwVisible" header="Set Sync Password" modal style="width: 380px">
        <p class="sync-desc">
            This password protects the shared sync file. Use the same password on every device you sync —
            it doesn't have to match your master password.
        </p>
        <Password
            v-model="autoSyncPwInput"
            :feedback="false"
            toggleMask
            fluid
            placeholder="Sync password"
            autofocus
            @keydown.enter="saveAutoSyncPassword"
        />
        <div class="dialog-footer">
            <Button label="Cancel" outlined @click="autoSyncPwVisible = false; autoSyncPwInput = ''" />
            <Button label="Save" :disabled="!autoSyncPwInput" @click="saveAutoSyncPassword" />
        </div>
    </Dialog>
</template>

<style scoped>
.settings-view { max-width: 680px; }
.page-title { font-size: 1.5rem; font-weight: 700; margin: 0 0 1.5rem; }

.section-card {
    border-radius: 12px;
    padding: 1.5rem;
    margin-bottom: 1.5rem;
    background: var(--p-content-background);
    border: 1px solid var(--p-content-border-color);
}
.section-card h2 { margin: 0 0 1.25rem; font-size: 1.1rem; font-weight: 600; }

/* Password form */
.pw-form { display: flex; flex-direction: column; gap: 1rem; }
.field { display: flex; flex-direction: column; gap: 0.4rem; }
label { font-size: 0.875rem; font-weight: 500; }

/* Theme row */
.theme-row { display: flex; justify-content: space-between; align-items: center; gap: 1.5rem; padding: 0.25rem 0; }

/* Auto-lock row */
.lock-row { display: flex; justify-content: space-between; align-items: center; gap: 1.5rem; padding: 0.25rem 0; }
.lock-control { display: flex; align-items: center; gap: 0.6rem; flex-shrink: 0; }

/* Data management rows */
.data-row { display: flex; justify-content: space-between; align-items: center; gap: 1.5rem; padding: 0.25rem 0; }
.data-row-info { display: flex; flex-direction: column; gap: 0.3rem; min-width: 0; }
.data-row > .p-toggleswitch,
.data-row > .p-button { flex-shrink: 0; }
.data-row-title { font-size: 0.95rem; font-weight: 500; }
.data-row-desc { font-size: 0.82rem; line-height: 1.4; color: var(--p-text-muted-color); }

/* About grid */
.about-grid {
    display: grid;
    grid-template-columns: 130px 1fr;
    row-gap: 0.6rem;
    column-gap: 1rem;
    font-size: 0.9rem;
    align-items: start;
}
.about-label { font-weight: 600; font-size: 0.82rem; padding-top: 0.05rem; color: var(--p-text-muted-color); }
.about-path { word-break: break-all; font-size: 0.83rem; font-family: monospace; }

/* Diagnostics */
.diag-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.5rem; }
.diag-actions { display: flex; gap: 0.5rem; }
.diag-desc { font-size: 0.82rem; color: var(--p-text-muted-color); margin: 0 0 1.25rem; line-height: 1.5; }
.diag-section-title { font-size: 0.9rem; font-weight: 600; margin: 0 0 0.6rem; color: var(--p-text-muted-color); text-transform: uppercase; letter-spacing: 0.04em; }
.diag-empty { font-size: 0.85rem; color: var(--p-text-muted-color); padding: 0.5rem 0; }
.diag-loading { display: flex; justify-content: center; padding: 1.5rem 0; }
.diag-table { font-size: 0.85rem; }
.error-list { display: flex; flex-direction: column; gap: 0.4rem; }
.error-entry { display: grid; grid-template-columns: 120px 1fr auto; gap: 0.5rem; align-items: baseline; font-size: 0.83rem; padding: 0.3rem 0; border-bottom: 1px solid var(--p-content-border-color); }
.error-type { font-weight: 600; color: var(--p-red-400); font-size: 0.78rem; }
.error-msg { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.error-time { font-size: 0.75rem; color: var(--p-text-muted-color); white-space: nowrap; }

/* Wipe dialog */
.mt-input { margin-top: 0.75rem; }
.dialog-footer { display: flex; justify-content: flex-end; gap: 0.75rem; margin-top: 1.25rem; }

/* Sync dialog */
.sync-desc { font-size: 0.85rem; color: var(--p-text-muted-color); margin: 0 0 1rem; line-height: 1.5; }

/* Auto sync config sub-rows */
.auto-sync-config { display: flex; flex-direction: column; gap: 0.6rem; padding: 0.5rem 0 0.25rem 0; }
.auto-sync-row { display: flex; justify-content: space-between; align-items: center; gap: 1rem; }
.auto-sync-label { font-size: 0.85rem; color: var(--p-text-muted-color); flex-shrink: 0; }
.auto-sync-control { display: flex; align-items: center; gap: 0.6rem; min-width: 0; }
.auto-sync-path { font-size: 0.83rem; font-family: monospace; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 220px; }

/* Developer section */
.dev-card { border-color: var(--p-orange-200); }
.dark .dev-card { border-color: var(--p-orange-800); }

@media (max-width: 639px) {
    .data-row { flex-direction: column; align-items: flex-start; gap: 0.75rem; }
    .lock-row { flex-direction: column; align-items: flex-start; gap: 0.75rem; }
    .theme-row { flex-direction: column; align-items: flex-start; gap: 0.75rem; }
    .auto-sync-row { flex-direction: column; align-items: flex-start; gap: 0.4rem; }
    .auto-sync-path { max-width: 100%; }
}
</style>
