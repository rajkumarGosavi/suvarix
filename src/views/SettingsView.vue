<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { save, open } from "@tauri-apps/plugin-dialog";
import { useConfirm } from "primevue/useconfirm";
import { useToast } from "primevue/usetoast";
import { useUiStore } from "@/stores/ui";
import type { Theme } from "@/stores/ui";

const ui = useUiStore();

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
            defaultPath: `finfolio-backup-${new Date().toISOString().slice(0, 10)}.db`,
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

// ─── About ───────────────────────────────────────────────────
const appDataDir = ref("");

onMounted(async () => {
    await loadLockSetting();
    try {
        appDataDir.value = await invoke<string>("get_app_data_dir");
    } catch { /* non-critical */ }
});

const APP_VERSION = "0.1.0";
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

        <!-- About -->
        <div class="section-card">
            <h2>About</h2>
            <div class="about-grid">
                <span class="about-label">App</span>
                <span>FinFolio — Personal Finance Tracker</span>
                <span class="about-label">Version</span>
                <span>{{ APP_VERSION }}</span>
                <span class="about-label">Data directory</span>
                <span class="about-path">{{ appDataDir || "—" }}</span>
                <span class="about-label">Privacy</span>
                <span>All data is stored locally on this device. Nothing is sent to the cloud.</span>
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

    <ConfirmDialog />
    <Toast />
</template>

<style scoped>
.settings-view { max-width: 680px; }
.page-title { font-size: 1.5rem; font-weight: 700; margin: 0 0 1.5rem; }

.section-card {
    border-radius: 12px;
    padding: 1.5rem;
    margin-bottom: 1.5rem;
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
.data-row-desc { font-size: 0.82rem; line-height: 1.4; }

/* About grid */
.about-grid {
    display: grid;
    grid-template-columns: 130px 1fr;
    row-gap: 0.6rem;
    column-gap: 1rem;
    font-size: 0.9rem;
    align-items: start;
}
.about-label { font-weight: 600; font-size: 0.82rem; padding-top: 0.05rem; }
.about-path { word-break: break-all; font-size: 0.83rem; font-family: monospace; }

/* Wipe dialog */
.mt-input { margin-top: 0.75rem; }
.dialog-footer { display: flex; justify-content: flex-end; gap: 0.75rem; margin-top: 1.25rem; }
</style>
