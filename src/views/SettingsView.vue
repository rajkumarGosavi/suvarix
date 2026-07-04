<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { save, open } from "@tauri-apps/plugin-dialog";
import { useConfirm } from "primevue/useconfirm";
import { useToast } from "primevue/usetoast";
import { useUiStore } from "@/stores/ui";
import type { Theme } from "@/stores/ui";
import { usePortfolioStore } from "@/stores/portfolio";
import { APP_NAME } from "@/constants";
import { useAnalytics } from "@/composables/useAnalytics";

// ─── Analytics types ─────────────────────────────────────────
interface EventStat   { eventName: string; count: number; lastSeen: string; }
interface ErrorEntry  { id: number; errorType: string; message: string; createdAt: string; }
interface PerfStat    { metricName: string; avgMs: number; count: number; }
interface AnalyticsExport { exportedAt: string; appVersion: string; events: EventStat[]; errors: ErrorEntry[]; perf: PerfStat[]; }

const ui = useUiStore();
const portfolio = usePortfolioStore();
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
        pwError.value = e?.message?.message ?? String(e?.message ?? "Failed to change password.");
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
        toast.add({ severity: "error", summary: "Backup failed", detail: String(e?.message ?? e), life: 5000 });
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
                toast.add({ severity: "error", summary: "Restore failed", detail: String(e?.message ?? e), life: 5000 });
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
        toast.add({ severity: "success", summary: "All data deleted", detail: "Portfolio and transaction data has been wiped.", life: 5000 });
    } catch (e: any) {
        toast.add({ severity: "error", summary: "Wipe failed", detail: String(e?.message ?? e), life: 5000 });
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
        toast.add({ severity: "error", summary: "Export failed", detail: String(e?.message ?? e), life: 5000 });
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
            toast.add({ severity: "success", summary: "Import complete", detail: `${r.rowsImported} records across ${r.tablesImported} tables restored. Restart the app to refresh all views.`, life: 8000 });
        }
    } catch (e: any) {
        const detail = String(e?.message ?? e);
        toast.add({ severity: "error", summary: syncMode.value === "export" ? "Export failed" : "Import failed", detail, life: 6000 });
        trackError(syncMode.value === "export" ? "sync_export_failed" : "sync_import_failed", detail);
    } finally {
        syncLoading.value = false;
        syncPassword.value = "";
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
            await portfolio.fetchAll();
            toast.add({ severity: "success", summary: "Dummy data loaded", detail: "Portfolio refreshed with demo records.", life: 4000 });
        } else {
            await invoke("clear_dummy_data");
            dummyDataEnabled.value = false;
            await portfolio.fetchAll();
            toast.add({ severity: "info", summary: "Dummy data cleared", detail: "All demo records removed.", life: 3000 });
        }
    } catch (e: any) {
        toast.add({ severity: "error", summary: "Failed", detail: String(e?.message ?? e), life: 4000 });
    } finally {
        dummyDataLoading.value = false;
    }
}

// ─── About ───────────────────────────────────────────────────
const appDataDir = ref("");

onMounted(async () => {
    await loadLockSetting();
    await loadDiagnostics();
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

/* Developer section */
.dev-card { border-color: var(--p-orange-200); }
.dark .dev-card { border-color: var(--p-orange-800); }

@media (max-width: 639px) {
    .data-row { flex-direction: column; align-items: flex-start; gap: 0.75rem; }
    .lock-row { flex-direction: column; align-items: flex-start; gap: 0.75rem; }
    .theme-row { flex-direction: column; align-items: flex-start; gap: 0.75rem; }
}
</style>
