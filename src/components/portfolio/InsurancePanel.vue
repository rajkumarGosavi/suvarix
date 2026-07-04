<script setup lang="ts">
import { reactive } from "vue";
import { useConfirm } from "primevue/useconfirm";
import { usePortfolioStore } from "@/stores/portfolio";
import { useHoldingCrud } from "@/composables/useHoldingCrud";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";
import { strToDate, dateToStr } from "@/composables/useDateConvert";

const portfolio = usePortfolioStore();
const confirm = useConfirm();
const { formatINR } = useCurrencyFormat();
const crud = useHoldingCrud("add_insurance", "update_insurance", "delete_insurance", portfolio.fetchInsurance.bind(portfolio));

interface InsuranceForm {
    insuranceType: string;
    provider: string;
    policyNumber: string;
    premiumAmount: number;
    premiumFreq: string;
    coverageAmount: number | null;
    maturityValue: number | null;
    startDate: Date | null;
    endDate: Date | null;
    nextDueDate: Date | null;
}

const form = reactive<InsuranceForm>({
    insuranceType: "term", provider: "", policyNumber: "", premiumAmount: 0,
    premiumFreq: "annual", coverageAmount: null, maturityValue: null,
    startDate: null, endDate: null, nextDueDate: null,
});

function resetForm() {
    Object.assign(form, {
        insuranceType: "term", provider: "", policyNumber: "", premiumAmount: 0,
        premiumFreq: "annual", coverageAmount: null, maturityValue: null,
        startDate: null, endDate: null, nextDueDate: null,
    });
}

function openAdd() {
    crud.editItem.value = null;
    resetForm();
    crud.showDialog.value = true;
}

function openEdit(item: any) {
    crud.editItem.value = { ...item };
    Object.assign(form, {
        insuranceType: item.insuranceType, provider: item.provider,
        policyNumber: item.policyNumber ?? "", premiumAmount: item.premiumAmount,
        premiumFreq: item.premiumFreq, coverageAmount: item.coverageAmount,
        maturityValue: item.maturityValue, startDate: strToDate(item.startDate),
        endDate: strToDate(item.endDate), nextDueDate: strToDate(item.nextDueDate),
    });
    crud.showDialog.value = true;
}

async function submit() {
    await crud.save({
        ...form,
        startDate: dateToStr(form.startDate) ?? "",
        endDate: dateToStr(form.endDate),
        nextDueDate: dateToStr(form.nextDueDate),
    });
}

function confirmDelete(item: any) {
    confirm.require({
        message: `Remove ${item.insuranceType} policy from ${item.provider}?`,
        header: "Delete Insurance",
        icon: "pi pi-trash",
        rejectProps: { label: "Cancel", outlined: true },
        acceptProps: { label: "Delete" },
        accept: () => crud.remove(item.id),
    });
}

function dueBadgeLabel(item: any) {
    if (!item.nextDueDate) return null;
    const days = Math.ceil((new Date(item.nextDueDate).getTime() - Date.now()) / 86400000);
    if (days < 0) return "Overdue";
    if (days <= 30) return `Due in ${days}d`;
    return item.nextDueDate;
}

const INS_TYPES = ["life", "health", "term", "ulip", "vehicle", "home"];
const FREQ_OPTIONS = ["monthly", "quarterly", "annual"];
</script>

<template>
    <div class="panel">
        <div class="panel-toolbar">
            <span class="count">{{ portfolio.insurance.length }} policies</span>
            <Button icon="pi pi-plus" label="Add Policy" size="small" @click="openAdd" />
        </div>

        <DataTable :value="portfolio.insurance" stripedRows emptyMessage="No insurance policies.">
            <Column field="insuranceType" header="Type">
                <template #body="{ data }">{{ data.insuranceType.toUpperCase() }}</template>
            </Column>
            <Column field="provider" header="Provider" sortable />
            <Column field="policyNumber" header="Policy No" />
            <Column header="Premium">
                <template #body="{ data }">{{ formatINR(data.premiumAmount) }} / {{ data.premiumFreq }}</template>
            </Column>
            <Column field="coverageAmount" header="Coverage">
                <template #body="{ data }">{{ data.coverageAmount ? formatINR(data.coverageAmount) : '—' }}</template>
            </Column>
            <Column field="maturityValue" header="Maturity Value">
                <template #body="{ data }">{{ data.maturityValue ? formatINR(data.maturityValue) : '—' }}</template>
            </Column>
            <Column header="Next Premium">
                <template #body="{ data }">
                    <Tag v-if="dueBadgeLabel(data)" :value="dueBadgeLabel(data)!" />
                    <span v-else>—</span>
                </template>
            </Column>
            <Column header="Actions" style="width:100px">
                <template #body="{ data }">
                    <Button icon="pi pi-pencil" text size="small" aria-label="Edit" @click="openEdit(data)" />
                    <Button icon="pi pi-trash" text size="small" aria-label="Delete" @click="confirmDelete(data)" />
                </template>
            </Column>
        </DataTable>
    </div>

    <Dialog v-model:visible="crud.showDialog.value" :header="crud.editItem.value ? 'Edit Policy' : 'Add Insurance Policy'" modal style="width:540px">
        <form @submit.prevent="submit" class="dialog-form">
            <div class="field-row">
                <div class="field">
                    <label>Type *</label>
                    <Select v-model="form.insuranceType" :options="INS_TYPES" class="w-full" />
                </div>
                <div class="field">
                    <label>Provider *</label>
                    <InputText v-model="form.provider" placeholder="LIC / HDFC Life" class="w-full" required />
                </div>
            </div>
            <div class="field">
                <label>Policy Number</label>
                <InputText v-model="form.policyNumber" class="w-full" />
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Premium Amount (₹) *</label>
                    <InputNumber v-model="form.premiumAmount" :min="0" class="w-full" required />
                </div>
                <div class="field">
                    <label>Frequency *</label>
                    <Select v-model="form.premiumFreq" :options="FREQ_OPTIONS" class="w-full" />
                </div>
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Coverage Amount (₹)</label>
                    <InputNumber v-model="form.coverageAmount" :min="0" class="w-full" />
                </div>
                <div class="field">
                    <label>Maturity Value (₹)</label>
                    <InputNumber v-model="form.maturityValue" :min="0" class="w-full" />
                </div>
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Start Date *</label>
                    <DatePicker v-model="form.startDate" dateFormat="dd/mm/yy" showIcon iconDisplay="input" class="w-full" required />
                </div>
                <div class="field">
                    <label>End Date</label>
                    <DatePicker v-model="form.endDate" dateFormat="dd/mm/yy" showIcon iconDisplay="input" class="w-full" />
                </div>
            </div>
            <div class="field">
                <label>Next Premium Due Date</label>
                <DatePicker v-model="form.nextDueDate" dateFormat="dd/mm/yy" showIcon iconDisplay="input" class="w-full" />
            </div>
            <div class="dialog-footer">
                <Button label="Cancel" outlined @click="crud.close()" />
                <Button type="submit" :label="crud.editItem.value ? 'Update' : 'Add'" :loading="crud.loading.value" />
            </div>
        </form>
    </Dialog>
</template>

<style scoped>
.panel-toolbar { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; }
.count { font-size: 0.875rem; }
.dialog-form { display: flex; flex-direction: column; gap: 1rem; padding: 0.5rem 0; }
.field { display: flex; flex-direction: column; gap: 0.4rem; flex: 1; }
.field-row { display: flex; gap: 1rem; }
label { font-size: 0.85rem; font-weight: 500; }
.dialog-footer { display: flex; justify-content: flex-end; gap: 0.75rem; margin-top: 0.5rem; }
</style>

