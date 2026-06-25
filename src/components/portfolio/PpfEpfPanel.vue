<script setup lang="ts">
import { computed } from "vue";
import { useConfirm } from "primevue/useconfirm";
import { usePortfolioStore } from "@/stores/portfolio";
import { useHoldingCrud } from "@/composables/useHoldingCrud";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";

const portfolio = usePortfolioStore();
const confirm = useConfirm();
const { formatINR } = useCurrencyFormat();
const { showDialog, editItem, loading, openAdd, openEdit, close, save, remove } =
    useHoldingCrud("add_ppf_epf", "update_ppf_epf", "delete_ppf_epf", portfolio.fetchPpfEpf.bind(portfolio));

const form = computed(() => editItem.value ?? {
    accountType: "PPF", accountNumber: "", balance: 0, interestRate: 7.1,
    financialYear: "", employerContrib: null, employeeContrib: null,
});

function confirmDelete(item: any) {
    confirm.require({
        message: `Remove ${item.accountType} account?`,
        header: "Delete Account",
        icon: "pi pi-trash",
        rejectProps: { label: "Cancel", outlined: true },
        acceptProps: { label: "Delete" },
        accept: () => remove(item.id),
    });
}
</script>

<template>
    <div class="panel">
        <div class="panel-toolbar">
            <span class="count">{{ portfolio.ppfEpf.length }} accounts</span>
            <Button icon="pi pi-plus" label="Add Account" size="small" @click="openAdd" />
        </div>

        <DataTable :value="portfolio.ppfEpf" stripedRows emptyMessage="No PPF/EPF accounts added.">
            <Column field="accountType" header="Type" />
            <Column field="accountNumber" header="Account No" />
            <Column field="balance" header="Balance" sortable>
                <template #body="{ data }">{{ formatINR(data.balance) }}</template>
            </Column>
            <Column field="interestRate" header="Rate">
                <template #body="{ data }">{{ data.interestRate }}%</template>
            </Column>
            <Column field="employerContrib" header="Employer Contrib">
                <template #body="{ data }">{{ data.employerContrib ? formatINR(data.employerContrib) : '—' }}</template>
            </Column>
            <Column field="employeeContrib" header="Employee Contrib">
                <template #body="{ data }">{{ data.employeeContrib ? formatINR(data.employeeContrib) : '—' }}</template>
            </Column>
            <Column header="" style="width:100px">
                <template #body="{ data }">
                    <Button icon="pi pi-pencil" text size="small" @click="openEdit(data)" />
                    <Button icon="pi pi-trash" text size="small" @click="confirmDelete(data)" />
                </template>
            </Column>
        </DataTable>
    </div>

    <Dialog v-model:visible="showDialog" :header="editItem ? 'Edit Account' : 'Add PPF/EPF Account'" modal style="width:480px">
        <form @submit.prevent="save(form)" class="dialog-form">
            <div class="field-row">
                <div class="field">
                    <label>Account Type *</label>
                    <Select v-model="form.accountType" :options="['PPF','EPF','NPS','VPF']" class="w-full" />
                </div>
                <div class="field">
                    <label>Interest Rate (%) *</label>
                    <InputNumber v-model="form.interestRate" :min="0" :max="20" :minFractionDigits="2" class="w-full" required />
                </div>
            </div>
            <div class="field">
                <label>Account Number</label>
                <InputText v-model="form.accountNumber" class="w-full" />
            </div>
            <div class="field">
                <label>Current Balance (₹) *</label>
                <InputNumber v-model="form.balance" :min="0" class="w-full" required />
            </div>
            <div class="field">
                <label>Financial Year</label>
                <InputText v-model="form.financialYear" placeholder="2024-25" class="w-full" />
            </div>
            <div class="field-row" v-if="form.accountType === 'EPF' || form.accountType === 'VPF'">
                <div class="field">
                    <label>Employer Contribution (₹)</label>
                    <InputNumber v-model="form.employerContrib" :min="0" class="w-full" />
                </div>
                <div class="field">
                    <label>Employee Contribution (₹)</label>
                    <InputNumber v-model="form.employeeContrib" :min="0" class="w-full" />
                </div>
            </div>
            <div class="dialog-footer">
                <Button label="Cancel" outlined @click="close" />
                <Button type="submit" :label="editItem ? 'Update' : 'Add'" :loading="loading" />
            </div>
        </form>
    </Dialog>
    <ConfirmDialog />
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
