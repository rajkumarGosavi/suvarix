<script setup lang="ts">
import { onMounted, ref, reactive } from "vue";
import { useConfirm } from "primevue/useconfirm";
import { useTransactionsStore } from "@/stores/transactions";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";
import { strToDate, dateToStr } from "@/composables/useDateConvert";

const store = useTransactionsStore();
const confirm = useConfirm();
const { formatINR } = useCurrencyFormat();

const showDialog = ref(false);
const editItem = ref<any>(null);
const loading = ref(false);

const TYPES = ["buy","sell","dividend","interest","sip","redemption","deposit","withdrawal","expense","income","emi","transfer"];
const ASSET_CLASSES = ["equity","mf","fd","ppf_epf","real_estate","gold","crypto","insurance","cash","loan","credit_card"];
const CATEGORIES = ["Food","Rent","EMI","Travel","Medical","Utilities","Entertainment","Education","Shopping","Dividend","Interest","Salary","Other"];

interface TxnForm {
    date: Date | null;
    type: string;
    assetClass: string | null;
    accountId: number | null;
    holdingId: number | null;
    amount: number;
    quantity: number | null;
    price: number | null;
    category: string | null;
    description: string;
    notes: string;
}

const form = reactive<TxnForm>({
    date: null, type: "expense", assetClass: null, accountId: null,
    holdingId: null, amount: 0, quantity: null, price: null,
    category: null, description: "", notes: "",
});

function resetForm() {
    Object.assign(form, {
        date: new Date(), type: "expense", assetClass: null, accountId: null,
        holdingId: null, amount: 0, quantity: null, price: null,
        category: null, description: "", notes: "",
    });
}

function openAdd() {
    editItem.value = null;
    resetForm();
    showDialog.value = true;
}

function openEdit(item: any) {
    editItem.value = item;
    Object.assign(form, {
        date: strToDate(item.date), type: item.type, assetClass: item.assetClass,
        accountId: item.accountId, holdingId: item.holdingId, amount: item.amount,
        quantity: item.quantity, price: item.price, category: item.category,
        description: item.description ?? "", notes: item.notes ?? "",
    });
    showDialog.value = true;
}

async function save() {
    loading.value = true;
    try {
        const payload = { ...form, date: dateToStr(form.date) ?? "" };
        if (editItem.value) {
            await store.update(editItem.value.id, payload);
        } else {
            await store.add(payload);
        }
        showDialog.value = false;
    } finally {
        loading.value = false;
    }
}

function confirmDelete(item: any) {
    confirm.require({
        message: `Delete this transaction (${item.description || item.type})?`,
        header: "Delete Transaction",
        icon: "pi pi-trash",
        rejectProps: { label: "Cancel", outlined: true },
        acceptProps: { label: "Delete" },
        accept: () => store.remove(item.id),
    });
}

const filterDateFrom = ref<Date | null>(null);
const filterType = ref<string | null>(null);

function applyFilter() {
    store.fetch({
        dateFrom: dateToStr(filterDateFrom.value) ?? undefined,
        category: filterType.value ?? undefined,
        limit: 100,
    });
}

function clearFilter() {
    filterDateFrom.value = null;
    filterType.value = null;
    store.fetch({ limit: 100 });
}

function isCredit(type: string) {
    return ["income","dividend","interest","sell","redemption","deposit"].includes(type);
}

onMounted(() => store.fetch({ limit: 100 }));
</script>

<template>
    <div class="transactions-view">
        <div class="page-header">
            <h1 class="page-title">Transactions</h1>
            <Button icon="pi pi-plus" label="Add Transaction" @click="openAdd" />
        </div>

        <div class="filter-bar">
            <DatePicker
                v-model="filterDateFrom"
                dateFormat="dd/mm/yy"
                showIcon
                iconDisplay="input"
                placeholder="From date"
                class="filter-input"
                @date-select="applyFilter"
            />
            <Select
                v-model="filterType"
                :options="TYPES"
                placeholder="All types"
                showClear
                class="filter-input"
                @change="applyFilter"
            />
            <Button icon="pi pi-times" text @click="clearFilter" v-tooltip="'Clear filters'" />
        </div>

        <ProgressSpinner v-if="store.isLoading" class="loading" />

        <DataTable
            v-else
            :value="store.transactions"
            stripedRows
            paginator
            :rows="25"
            emptyMessage="No transactions yet. Click Add to record one."
        >
            <Column field="date" header="Date" sortable style="width:110px" />
            <Column field="type" header="Type" style="width:120px">
                <template #body="{ data }">
                    <Tag :value="data.type" />
                </template>
            </Column>
            <Column field="assetClass" header="Asset Class" />
            <Column field="description" header="Description" />
            <Column field="category" header="Category" />
            <Column field="amount" header="Amount" sortable style="width:140px">
                <template #body="{ data }">
                    {{ isCredit(data.type) ? "+" : "−" }}{{ formatINR(Math.abs(data.amount)) }}
                </template>
            </Column>
            <Column header="" style="width:90px">
                <template #body="{ data }">
                    <Button icon="pi pi-pencil" text size="small" @click="openEdit(data)" />
                    <Button icon="pi pi-trash" text size="small" @click="confirmDelete(data)" />
                </template>
            </Column>
        </DataTable>
    </div>

    <Dialog v-model:visible="showDialog" :header="editItem ? 'Edit Transaction' : 'Add Transaction'" modal style="width:520px">
        <form @submit.prevent="save" class="dialog-form">
            <div class="field-row">
                <div class="field">
                    <label>Date *</label>
                    <DatePicker v-model="form.date" dateFormat="dd/mm/yy" showIcon iconDisplay="input" class="w-full" required />
                </div>
                <div class="field">
                    <label>Type *</label>
                    <Select v-model="form.type" :options="TYPES" class="w-full" required />
                </div>
            </div>
            <div class="field">
                <label>Amount (₹) *</label>
                <InputNumber v-model="form.amount" :min="0" :minFractionDigits="2" class="w-full" required />
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Category</label>
                    <Select v-model="form.category" :options="CATEGORIES" placeholder="Select…" showClear class="w-full" />
                </div>
                <div class="field">
                    <label>Asset Class</label>
                    <Select v-model="form.assetClass" :options="ASSET_CLASSES" placeholder="Select…" showClear class="w-full" />
                </div>
            </div>
            <div class="field">
                <label>Description</label>
                <InputText v-model="form.description" class="w-full" />
            </div>
            <div class="field">
                <label>Notes</label>
                <Textarea v-model="form.notes" :rows="2" autoResize class="w-full" />
            </div>
            <div class="dialog-footer">
                <Button label="Cancel" outlined @click="showDialog = false" />
                <Button type="submit" :label="editItem ? 'Update' : 'Add'" :loading="loading" />
            </div>
        </form>
    </Dialog>
    <ConfirmDialog />
</template>

<style scoped>
.transactions-view { max-width: 1100px; }
.page-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.5rem; flex-wrap: wrap; gap: 1rem; }
.page-title { font-size: 1.5rem; font-weight: 700; margin: 0; }
.filter-bar { display: flex; gap: 0.75rem; margin-bottom: 1.25rem; flex-wrap: wrap; align-items: center; }
.filter-input { min-width: 180px; }
.loading { display: flex; justify-content: center; padding: 3rem; }
.dialog-form { display: flex; flex-direction: column; gap: 1rem; padding: 0.5rem 0; }
.field { display: flex; flex-direction: column; gap: 0.4rem; flex: 1; }
.field-row { display: flex; gap: 1rem; }
label { font-size: 0.85rem; font-weight: 500; }
.dialog-footer { display: flex; justify-content: flex-end; gap: 0.75rem; margin-top: 0.5rem; }
</style>
