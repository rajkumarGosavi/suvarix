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
const crud = useHoldingCrud("add_fd", "update_fd", "delete_fd", portfolio.fetchFd.bind(portfolio));

interface FdForm {
    accountId: number | null;
    bankName: string;
    accountNumber: string;
    principal: number;
    interestRate: number;
    compounding: string;
    tenureMonths: number;
    startDate: Date | null;
    maturityDate: Date | null;
    maturityAmount: number | null;
    isCumulative: boolean;
}

const form = reactive<FdForm>({
    accountId: null, bankName: "", accountNumber: "", principal: 0,
    interestRate: 0, compounding: "quarterly", tenureMonths: 12,
    startDate: null, maturityDate: null, maturityAmount: null, isCumulative: true,
});

function resetForm() {
    Object.assign(form, {
        accountId: null, bankName: "", accountNumber: "", principal: 0,
        interestRate: 0, compounding: "quarterly", tenureMonths: 12,
        startDate: null, maturityDate: null, maturityAmount: null, isCumulative: true,
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
        accountId: item.accountId, bankName: item.bankName, accountNumber: item.accountNumber ?? "",
        principal: item.principal, interestRate: item.interestRate, compounding: item.compounding,
        tenureMonths: item.tenureMonths, startDate: strToDate(item.startDate),
        maturityDate: strToDate(item.maturityDate), maturityAmount: item.maturityAmount,
        isCumulative: item.isCumulative,
    });
    crud.showDialog.value = true;
}

async function submit() {
    await crud.save({
        ...form,
        startDate: dateToStr(form.startDate) ?? "",
        maturityDate: dateToStr(form.maturityDate) ?? "",
    });
}

function confirmDelete(item: any) {
    confirm.require({
        message: `Remove FD with ${item.bankName}?`,
        header: "Delete Fixed Deposit",
        icon: "pi pi-trash",
        rejectProps: { label: "Cancel", outlined: true },
        acceptProps: { label: "Delete" },
        accept: () => crud.remove(item.id),
    });
}

const COMPOUNDING = ["monthly", "quarterly", "half-yearly", "annually"];

function daysToMaturity(maturityDate: string) {
    if (!maturityDate) return "—";
    const diff = new Date(maturityDate).getTime() - Date.now();
    const days = Math.ceil(diff / 86400000);
    if (days < 0) return "Matured";
    if (days < 30) return `${days}d`;
    return `${Math.floor(days / 30)}mo`;
}
</script>

<template>
    <div class="panel">
        <div class="panel-toolbar">
            <span class="count">{{ portfolio.fd.length }} fixed deposits</span>
            <Button icon="pi pi-plus" label="Add FD" size="small" @click="openAdd" />
        </div>

        <DataTable :value="portfolio.fd" stripedRows emptyMessage="No fixed deposits added.">
            <Column field="bankName" header="Bank" sortable />
            <Column field="principal" header="Principal" sortable>
                <template #body="{ data }">{{ formatINR(data.principal) }}</template>
            </Column>
            <Column field="interestRate" header="Rate">
                <template #body="{ data }">{{ data.interestRate }}%</template>
            </Column>
            <Column field="compounding" header="Compounding" />
            <Column field="maturityAmount" header="Maturity Amt">
                <template #body="{ data }">{{ data.maturityAmount ? formatINR(data.maturityAmount) : '—' }}</template>
            </Column>
            <Column header="Matures In" style="width:130px">
                <template #body="{ data }">
                    <Tag :value="daysToMaturity(data.maturityDate)" />
                </template>
            </Column>
            <Column field="maturityDate" header="Date" />
            <Column header="Actions" style="width:100px">
                <template #body="{ data }">
                    <Button icon="pi pi-pencil" text size="small" aria-label="Edit" @click="openEdit(data)" />
                    <Button icon="pi pi-trash" text size="small" aria-label="Delete" @click="confirmDelete(data)" />
                </template>
            </Column>
        </DataTable>
    </div>

    <Dialog v-model:visible="crud.showDialog.value" :header="crud.editItem.value ? 'Edit FD' : 'Add Fixed Deposit'" modal style="width:520px">
        <form @submit.prevent="submit" class="dialog-form">
            <div class="field-row">
                <div class="field">
                    <label>Bank Name *</label>
                    <InputText v-model="form.bankName" placeholder="HDFC Bank" class="w-full" required />
                </div>
                <div class="field">
                    <label>Account Number</label>
                    <InputText v-model="form.accountNumber" class="w-full" />
                </div>
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Principal (₹) *</label>
                    <InputNumber v-model="form.principal" :min="0" class="w-full" required />
                </div>
                <div class="field">
                    <label>Interest Rate (%) *</label>
                    <InputNumber v-model="form.interestRate" :min="0" :max="30" :minFractionDigits="2" class="w-full" required />
                </div>
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Compounding</label>
                    <Select v-model="form.compounding" :options="COMPOUNDING" class="w-full" />
                </div>
                <div class="field">
                    <label>Tenure (months) *</label>
                    <InputNumber v-model="form.tenureMonths" :min="1" class="w-full" required />
                </div>
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Start Date *</label>
                    <DatePicker v-model="form.startDate" dateFormat="dd/mm/yy" showIcon iconDisplay="input" class="w-full" required />
                </div>
                <div class="field">
                    <label>Maturity Date *</label>
                    <DatePicker v-model="form.maturityDate" dateFormat="dd/mm/yy" showIcon iconDisplay="input" class="w-full" required />
                </div>
            </div>
            <div class="field">
                <label>Maturity Amount (₹)</label>
                <InputNumber v-model="form.maturityAmount" :min="0" class="w-full" />
            </div>
            <div class="field-row field--check-row">
                <Checkbox v-model="form.isCumulative" binary inputId="cumulative" />
                <label for="cumulative">Cumulative (interest reinvested)</label>
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
.field--check-row { flex-direction: row; align-items: center; gap: 0.5rem; }
label { font-size: 0.85rem; font-weight: 500; }
.dialog-footer { display: flex; justify-content: flex-end; gap: 0.75rem; margin-top: 0.5rem; }
</style>

