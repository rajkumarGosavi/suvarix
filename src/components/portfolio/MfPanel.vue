<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useConfirm } from "primevue/useconfirm";
import { useToast } from "primevue/usetoast";
import { usePortfolioStore } from "@/stores/portfolio";
import { useHoldingCrud } from "@/composables/useHoldingCrud";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";
import { strToDate, dateToStr } from "@/composables/useDateConvert";

const portfolio = usePortfolioStore();
const confirm = useConfirm();
const toast = useToast();
const { formatINR, formatPercent } = useCurrencyFormat();
const { showDialog, editItem, loading, openAdd, openEdit, close, save, remove } =
    useHoldingCrud("add_mf", "update_mf", "delete_mf", portfolio.fetchMf.bind(portfolio));

const form = computed(() => editItem.value ?? {
    accountId: null, schemeCode: "", schemeName: "", amcName: "", folioNumber: "",
    units: 0, avgNav: 0, isDirect: true, isGrowth: true,
});

const holdings = computed(() => portfolio.mf.map((h: any) => ({
    ...h,
    currentValue: h.units * (h.currentNav ?? h.avgNav),
    investedValue: h.units * h.avgNav,
    get pnl() { return this.currentValue - this.investedValue; },
    get pnlPct() { return this.investedValue > 0 ? (this.pnl / this.investedValue) * 100 : 0; },
})));

function confirmDelete(item: any) {
    confirm.require({
        message: `Remove ${item.schemeName}?`,
        header: "Delete MF Holding",
        icon: "pi pi-trash",
        rejectProps: { label: "Cancel", outlined: true },
        acceptProps: { label: "Delete" },
        accept: () => remove(item.id),
    });
}

// ─── SIP Schedules ────────────────────────────────────────────
const FREQUENCIES = ["monthly", "quarterly", "weekly"];

interface SipSchedule {
    id: number;
    accountId: number;
    mfHoldingId: number | null;
    schemeCode: string;
    schemeName: string | null;
    amount: number;
    frequency: string;
    debitDay: number;
    startDate: string;
    endDate: string | null;
    isActive: boolean;
}

interface SipForm {
    accountId: number | null;
    mfHoldingId: number | null;
    schemeCode: string;
    amount: number;
    frequency: string;
    debitDay: number;
    startDate: Date | null;
    endDate: Date | null;
    isActive: boolean;
}

const sips = ref<SipSchedule[]>([]);
const sipLoading = ref(false);
const showSipDialog = ref(false);
const editSip = ref<SipSchedule | null>(null);
const sipFormLoading = ref(false);

const sipForm = reactive<SipForm>({
    accountId: null, mfHoldingId: null, schemeCode: "", amount: 0,
    frequency: "monthly", debitDay: 1,
    startDate: null, endDate: null, isActive: true,
});

async function fetchSips() {
    sipLoading.value = true;
    try { sips.value = await invoke<SipSchedule[]>("list_sip_schedules"); }
    finally { sipLoading.value = false; }
}

function resetSipForm() {
    Object.assign(sipForm, {
        accountId: null, mfHoldingId: null, schemeCode: "", amount: 0,
        frequency: "monthly", debitDay: 1,
        startDate: new Date(), endDate: null, isActive: true,
    });
}

const mfHoldingOptions = computed(() =>
    portfolio.mf.map((h: any) => ({
        label: h.schemeName ?? h.schemeCode,
        value: h.id,
        accountId: h.accountId,
        schemeCode: h.schemeCode,
    }))
);

function onHoldingSelect(holdingId: number | null) {
    const h = mfHoldingOptions.value.find(o => o.value === holdingId);
    if (h) {
        sipForm.accountId = h.accountId;
        if (!sipForm.schemeCode) sipForm.schemeCode = h.schemeCode;
    }
}

function openAddSip() {
    editSip.value = null;
    resetSipForm();
    showSipDialog.value = true;
}

function openEditSip(item: SipSchedule) {
    editSip.value = item;
    Object.assign(sipForm, {
        accountId: item.accountId,
        mfHoldingId: item.mfHoldingId,
        schemeCode: item.schemeCode,
        amount: item.amount,
        frequency: item.frequency,
        debitDay: item.debitDay,
        startDate: strToDate(item.startDate),
        endDate: strToDate(item.endDate ?? ""),
        isActive: item.isActive,
    });
    showSipDialog.value = true;
}

async function submitSip() {
    if (!sipForm.accountId) {
        toast.add({ severity: "warn", summary: "Select a holding", detail: "Pick an MF holding to link this SIP to an account.", life: 4000 });
        return;
    }
    sipFormLoading.value = true;
    try {
        const payload = {
            accountId: sipForm.accountId,
            mfHoldingId: sipForm.mfHoldingId,
            schemeCode: sipForm.schemeCode,
            amount: sipForm.amount,
            frequency: sipForm.frequency,
            debitDay: sipForm.debitDay,
            startDate: dateToStr(sipForm.startDate) ?? "",
            endDate: dateToStr(sipForm.endDate) ?? null,
            isActive: sipForm.isActive,
        };
        if (editSip.value) {
            await invoke("update_sip_schedule", { id: editSip.value.id, payload });
        } else {
            await invoke("add_sip_schedule", { payload });
        }
        showSipDialog.value = false;
        await fetchSips();
    } catch (e: any) {
        toast.add({ severity: "error", summary: "Failed to save SIP", detail: String(e), life: 5000 });
    } finally {
        sipFormLoading.value = false;
    }
}

function confirmDeleteSip(item: SipSchedule) {
    const label = item.schemeName ?? item.schemeCode;
    confirm.require({
        message: `Delete SIP for ${label}?`,
        header: "Delete SIP",
        icon: "pi pi-trash",
        rejectProps: { label: "Cancel", outlined: true },
        acceptProps: { label: "Delete" },
        accept: async () => {
            await invoke("delete_sip_schedule", { id: item.id });
            await fetchSips();
        },
    });
}

function nextSipDate(sip: SipSchedule): string {
    const today = new Date();
    const day = sip.debitDay;
    let candidate = new Date(today.getFullYear(), today.getMonth(), day);
    if (candidate <= today) {
        if (sip.frequency === "weekly") {
            candidate = new Date(today.getTime() + 7 * 86400000);
        } else if (sip.frequency === "quarterly") {
            candidate = new Date(today.getFullYear(), today.getMonth() + 3, day);
        } else {
            candidate = new Date(today.getFullYear(), today.getMonth() + 1, day);
        }
    }
    return candidate.toLocaleDateString("en-IN", { day: "2-digit", month: "short", year: "numeric" });
}

onMounted(() => {
    portfolio.fetchMf();
    fetchSips();
});
</script>

<template>
    <div class="panel">
        <!-- MF Holdings -->
        <div class="panel-toolbar">
            <span class="count">{{ holdings.length }} holdings</span>
            <Button icon="pi pi-plus" label="Add MF" size="small" @click="openAdd" />
        </div>

        <DataTable :value="holdings" stripedRows emptyMessage="No mutual fund holdings.">
            <Column field="schemeName" header="Scheme" sortable />
            <Column field="amcName" header="AMC" />
            <Column field="units" header="Units">
                <template #body="{ data }">{{ data.units.toFixed(3) }}</template>
            </Column>
            <Column field="avgNav" header="Avg NAV">
                <template #body="{ data }">{{ formatINR(data.avgNav) }}</template>
            </Column>
            <Column field="currentValue" header="Value" sortable>
                <template #body="{ data }">{{ formatINR(data.currentValue) }}</template>
            </Column>
            <Column field="pnl" header="P&amp;L" sortable>
                <template #body="{ data }">
                    <span :class="data.pnl >= 0 ? 'gain' : 'loss'">
                        {{ formatINR(data.pnl) }} ({{ formatPercent(data.pnlPct) }})
                    </span>
                </template>
            </Column>
            <Column field="isDirect" header="Type">
                <template #body="{ data }">
                    <Tag :value="data.isDirect ? 'Direct' : 'Regular'" />
                </template>
            </Column>
            <Column header="Actions" style="width:100px">
                <template #body="{ data }">
                    <Button icon="pi pi-pencil" text size="small" aria-label="Edit" @click="openEdit(data)" />
                    <Button icon="pi pi-trash" text size="small" aria-label="Delete" @click="confirmDelete(data)" />
                </template>
            </Column>
        </DataTable>

        <!-- SIP Schedules -->
        <div class="sip-header">
            <h3>SIP Schedules</h3>
            <Button icon="pi pi-plus" label="Add SIP" size="small" @click="openAddSip" />
        </div>

        <ProgressSpinner v-if="sipLoading" class="sip-loading" />

        <DataTable v-else :value="sips" stripedRows emptyMessage="No SIPs added. Click Add SIP to set one up.">
            <Column header="Scheme" style="min-width:160px">
                <template #body="{ data }">{{ data.schemeName ?? data.schemeCode }}</template>
            </Column>
            <Column field="amount" header="Amount">
                <template #body="{ data }">{{ formatINR(data.amount) }}</template>
            </Column>
            <Column field="frequency" header="Frequency" style="width:110px">
                <template #body="{ data }">
                    <Tag :value="data.frequency" />
                </template>
            </Column>
            <Column header="Next SIP" style="width:140px">
                <template #body="{ data }">
                    <span v-if="data.isActive">{{ nextSipDate(data) }}</span>
                    <Tag v-else value="Paused" />
                </template>
            </Column>
            <Column field="startDate" header="Start" style="width:105px" />
            <Column field="endDate" header="End" style="width:105px">
                <template #body="{ data }">{{ data.endDate ?? '—' }}</template>
            </Column>
            <Column header="Actions" style="width:90px">
                <template #body="{ data }">
                    <Button icon="pi pi-pencil" text size="small" aria-label="Edit SIP" @click="openEditSip(data)" />
                    <Button icon="pi pi-trash" text size="small" aria-label="Delete SIP" @click="confirmDeleteSip(data)" />
                </template>
            </Column>
        </DataTable>
    </div>

    <!-- MF Holding Dialog -->
    <Dialog v-model:visible="showDialog" :header="editItem ? 'Edit MF Holding' : 'Add MF Holding'" modal style="width:520px">
        <form @submit.prevent="save(form)" class="dialog-form">
            <div class="field">
                <label>Scheme Name *</label>
                <InputText v-model="form.schemeName" placeholder="Mirae Asset Large Cap Fund" class="w-full" required />
            </div>
            <div class="field-row">
                <div class="field">
                    <label>AMC Name *</label>
                    <InputText v-model="form.amcName" placeholder="Mirae Asset" class="w-full" required />
                </div>
                <div class="field">
                    <label>Scheme Code</label>
                    <InputText v-model="form.schemeCode" placeholder="118989" class="w-full" />
                </div>
            </div>
            <div class="field">
                <label>Folio Number *</label>
                <InputText v-model="form.folioNumber" class="w-full" required />
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Units *</label>
                    <InputNumber v-model="form.units" :min="0" :minFractionDigits="3" class="w-full" required />
                </div>
                <div class="field">
                    <label>Avg NAV (₹) *</label>
                    <InputNumber v-model="form.avgNav" :min="0" :minFractionDigits="4" class="w-full" required />
                </div>
            </div>
            <div class="field-row">
                <div class="field field--check">
                    <Checkbox v-model="form.isDirect" binary inputId="isDirect" />
                    <label for="isDirect">Direct Plan</label>
                </div>
                <div class="field field--check">
                    <Checkbox v-model="form.isGrowth" binary inputId="isGrowth" />
                    <label for="isGrowth">Growth Option</label>
                </div>
            </div>
            <div class="dialog-footer">
                <Button label="Cancel" outlined @click="close" />
                <Button type="submit" :label="editItem ? 'Update' : 'Add'" :loading="loading" />
            </div>
        </form>
    </Dialog>

    <!-- SIP Dialog -->
    <Dialog v-model:visible="showSipDialog" :header="editSip ? 'Edit SIP' : 'Add SIP'" modal style="width:480px">
        <form @submit.prevent="submitSip" class="dialog-form">
            <div class="field">
                <label>Linked MF Holding *</label>
                <Select
                    v-model="sipForm.mfHoldingId"
                    :options="mfHoldingOptions"
                    optionLabel="label"
                    optionValue="value"
                    placeholder="Select a holding"
                    class="w-full"
                    @change="onHoldingSelect(sipForm.mfHoldingId)"
                />
            </div>
            <div class="field">
                <label>Scheme Name / Code *</label>
                <InputText v-model="sipForm.schemeCode" placeholder="e.g. Mirae Asset Large Cap or scheme code" class="w-full" required />
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Amount (₹) *</label>
                    <InputNumber v-model="sipForm.amount" :min="100" :minFractionDigits="0" class="w-full" required />
                </div>
                <div class="field">
                    <label>Frequency *</label>
                    <Select v-model="sipForm.frequency" :options="FREQUENCIES" class="w-full" required />
                </div>
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Debit Day (1–31) *</label>
                    <InputNumber v-model="sipForm.debitDay" :min="1" :max="31" class="w-full" required />
                </div>
                <div class="field">
                    <label>Start Date *</label>
                    <DatePicker v-model="sipForm.startDate" dateFormat="dd/mm/yy" showIcon iconDisplay="input" class="w-full" required />
                </div>
            </div>
            <div class="field">
                <label>End Date (optional)</label>
                <DatePicker v-model="sipForm.endDate" dateFormat="dd/mm/yy" showIcon iconDisplay="input" class="w-full" />
            </div>
            <div class="field field--check">
                <Checkbox v-model="sipForm.isActive" binary inputId="sipActive" />
                <label for="sipActive">Active</label>
            </div>
            <div class="dialog-footer">
                <Button label="Cancel" outlined @click="showSipDialog = false" />
                <Button type="submit" :label="editSip ? 'Update' : 'Add SIP'" :loading="sipFormLoading" />
            </div>
        </form>
    </Dialog>
</template>

<style scoped>
.panel-toolbar { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; }
.count { font-size: 0.875rem; }

.sip-header {
    display: flex; justify-content: space-between; align-items: center;
    margin: 1.75rem 0 0.75rem;
}
.sip-header h3 { margin: 0; font-size: 1rem; font-weight: 600; }
.sip-loading { display: flex; justify-content: center; padding: 2rem; }

.dialog-form { display: flex; flex-direction: column; gap: 1rem; padding: 0.5rem 0; }
.field { display: flex; flex-direction: column; gap: 0.4rem; flex: 1; }
.field--check { flex-direction: row; align-items: center; gap: 0.5rem; }
.field-row { display: flex; gap: 1rem; }
label { font-size: 0.85rem; font-weight: 500; }
.dialog-footer { display: flex; justify-content: flex-end; gap: 0.75rem; margin-top: 0.5rem; }
</style>

