<script setup lang="ts">
import { onMounted, ref, reactive, computed } from "vue";
import { useConfirm } from "primevue/useconfirm";
import { useTransactionsStore } from "@/stores/transactions";
import { useCategoriesStore } from "@/stores/categories";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";
import { dateToStr, strToDateTime, dateTimeToStr } from "@/composables/useDateConvert";
import { useGamificationSafe } from "@/composables/useGamification";
import CategoryManagerDialog from "@/components/CategoryManagerDialog.vue";

const store = useTransactionsStore();
const categoriesStore = useCategoriesStore();
const confirm = useConfirm();
const { formatINR } = useCurrencyFormat();
const { awardXP, updateStreak } = useGamificationSafe();

const showDialog = ref(false);
const showCategoryManager = ref(false);
const editItem = ref<any>(null);
const loading = ref(false);

const TYPES = ["buy","sell","dividend","interest","sip","redemption","deposit","withdrawal","expense","income","emi","transfer"];
const ASSET_CLASSES = ["equity","mf","fd","ppf_epf","real_estate","gold","crypto","insurance","cash","loan","credit_card"];

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
    tag: string;
    description: string;
    notes: string;
}

const form = reactive<TxnForm>({
    date: null, type: "expense", assetClass: null, accountId: null,
    holdingId: null, amount: 0, quantity: null, price: null,
    category: null, tag: "", description: "", notes: "",
});

function resetForm() {
    Object.assign(form, {
        date: new Date(), type: "expense", assetClass: null, accountId: null,
        holdingId: null, amount: 0, quantity: null, price: null,
        category: null, tag: "", description: "", notes: "",
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
        date: strToDateTime(item.date), type: item.type, assetClass: item.assetClass,
        accountId: item.accountId, holdingId: item.holdingId, amount: item.amount,
        quantity: item.quantity, price: item.price, category: item.category,
        tag: item.tag ?? "", description: item.description ?? "", notes: item.notes ?? "",
    });
    showDialog.value = true;
}

async function save() {
    loading.value = true;
    try {
        const payload = { ...form, date: dateTimeToStr(form.date) ?? "", tag: form.tag.trim() || null };
        if (editItem.value) {
            await store.update(editItem.value.id, payload);
        } else {
            await store.add(payload);
            await awardXP("transaction_logged", 5);
            await updateStreak("transaction");
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

const PAGE_SIZE = 25;
const filterDateFrom = ref<Date | null>(null);
const filterType = ref<string | null>(null);
const searchQuery = ref("");
const sortField = ref<"date" | "amount">("date");
const sortOrder = ref<1 | -1>(-1); // PrimeVue convention: 1 = asc, -1 = desc

const currentOffset = ref(0);
const totalPages = computed(() => Math.max(1, Math.ceil(store.totalCount / PAGE_SIZE)));
const currentPage = computed(() => Math.floor(currentOffset.value / PAGE_SIZE) + 1);

function fetchPage(offset: number) {
    currentOffset.value = offset;
    store.fetch({
        dateFrom: dateToStr(filterDateFrom.value) ?? undefined,
        type: filterType.value ?? undefined,
        search: searchQuery.value.trim() || undefined,
        sortBy: sortField.value,
        sortDir: sortOrder.value === 1 ? "asc" : "desc",
        limit: PAGE_SIZE,
        offset,
    });
}

function applyFilter() {
    fetchPage(0);
}

let searchDebounce: ReturnType<typeof setTimeout> | undefined;
function onSearchInput() {
    clearTimeout(searchDebounce);
    searchDebounce = setTimeout(() => fetchPage(0), 350);
}

function clearFilter() {
    filterDateFrom.value = null;
    filterType.value = null;
    searchQuery.value = "";
    fetchPage(0);
}

function onPage(event: { first: number; rows: number }) {
    fetchPage(event.first);
}

function onSort(event: { sortField?: string | ((item: any) => string); sortOrder?: 1 | 0 | -1 | null }) {
    sortField.value = event.sortField === "amount" ? "amount" : "date";
    sortOrder.value = event.sortOrder === 1 ? 1 : -1;
    fetchPage(0);
}

function isCredit(type: string) {
    return ["income","dividend","interest","sell","redemption","deposit"].includes(type);
}

function formatDateTime(s: string) {
    const d = strToDateTime(s);
    if (!d) return s;
    return d.toLocaleString("en-IN", {
        day: "2-digit", month: "short", year: "numeric",
        hour: "2-digit", minute: "2-digit",
    });
}

onMounted(() => {
    fetchPage(0);
    categoriesStore.fetchCategories();
});
</script>

<template>
    <div class="transactions-view">
        <div class="page-header">
            <h1 class="page-title">Transactions</h1>
            <Button icon="pi pi-plus" label="Add Transaction" @click="openAdd" />
        </div>

        <div class="filter-bar">
            <IconField class="filter-input">
                <InputIcon class="pi pi-search" />
                <InputText
                    v-model="searchQuery"
                    placeholder="Search description, category, or tag…"
                    class="w-full"
                    @input="onSearchInput"
                />
            </IconField>
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

        <p v-if="store.totalCount" class="page-count-hint">
            {{ store.totalCount }} transaction{{ store.totalCount !== 1 ? 's' : '' }} — page {{ currentPage }} of {{ totalPages }}
        </p>

        <ProgressSpinner v-if="store.isLoading" class="loading" />

        <DataTable
            v-else
            :value="store.transactions"
            stripedRows
            paginator
            lazy
            :rows="PAGE_SIZE"
            :totalRecords="store.totalCount"
            @page="onPage"
            sortMode="single"
            :sortField="sortField"
            :sortOrder="sortOrder"
            @sort="onSort"
            emptyMessage="No transactions yet. Click Add to record one."
        >
            <Column field="date" header="Date" style="width:160px">
                <template #body="{ data }">{{ formatDateTime(data.date) }}</template>
            </Column>
            <Column field="type" header="Type" style="width:120px">
                <template #body="{ data }">
                    <Tag :value="data.type" />
                </template>
            </Column>
            <Column field="assetClass" header="Asset Class" />
            <Column field="description" header="Description" />
            <Column field="category" header="Category" />
            <Column field="tag" header="Tag" style="width:110px">
                <template #body="{ data }">
                    <Tag v-if="data.tag" :value="data.tag" severity="secondary" />
                </template>
            </Column>
            <Column field="amount" header="Amount" sortable style="width:140px">
                <template #body="{ data }">
                    {{ isCredit(data.type) ? "+" : "−" }}{{ formatINR(Math.abs(data.amount)) }}
                </template>
            </Column>
            <Column header="Actions" style="width:90px">
                <template #body="{ data }">
                    <Button icon="pi pi-pencil" text size="small" aria-label="Edit transaction" @click="openEdit(data)" />
                    <Button icon="pi pi-trash" text size="small" aria-label="Delete transaction" @click="confirmDelete(data)" />
                </template>
            </Column>
        </DataTable>
    </div>

    <Dialog v-model:visible="showDialog" :header="editItem ? 'Edit Transaction' : 'Add Transaction'" modal style="width:520px">
        <form @submit.prevent="save" class="dialog-form">
            <div class="field-row">
                <div class="field">
                    <label>Date *</label>
                    <DatePicker v-model="form.date" dateFormat="dd/mm/yy" showTime hourFormat="24" showIcon iconDisplay="input" class="w-full" required />
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
                    <div class="category-field-row">
                        <Select v-model="form.category" :options="categoriesStore.names" placeholder="Select…" showClear class="w-full" />
                        <Button icon="pi pi-cog" text aria-label="Manage categories" v-tooltip="'Manage categories'" @click="showCategoryManager = true" />
                    </div>
                </div>
                <div class="field">
                    <label>Asset Class</label>
                    <Select v-model="form.assetClass" :options="ASSET_CLASSES" placeholder="Select…" showClear class="w-full" />
                </div>
            </div>
            <div class="field">
                <label>Tag</label>
                <InputText v-model="form.tag" placeholder="e.g. House, Personal…" class="w-full" />
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

    <CategoryManagerDialog v-model:visible="showCategoryManager" />
</template>

<style scoped>
.transactions-view { max-width: 1100px; }
.page-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.5rem; flex-wrap: wrap; gap: 1rem; }
.page-title { font-size: 1.5rem; font-weight: 700; margin: 0; }
.filter-bar { display: flex; gap: 0.75rem; margin-bottom: 1.25rem; flex-wrap: wrap; align-items: center; }
.filter-input { min-width: 180px; }
.page-count-hint { font-size: 0.85rem; color: var(--p-text-muted-color); margin: -0.5rem 0 1rem; }
.loading { display: flex; justify-content: center; padding: 3rem; }
.dialog-form { display: flex; flex-direction: column; gap: 1rem; padding: 0.5rem 0; }
.field { display: flex; flex-direction: column; gap: 0.4rem; flex: 1; }
.field-row { display: flex; gap: 1rem; }
.category-field-row { display: flex; gap: 0.4rem; align-items: center; }
.category-field-row .p-select { flex: 1; }
label { font-size: 0.85rem; font-weight: 500; }
.dialog-footer { display: flex; justify-content: flex-end; gap: 0.75rem; margin-top: 0.5rem; }

@media (max-width: 639px) {
    .filter-bar { flex-direction: column; align-items: stretch; }
    .filter-input { min-width: unset; }
    .field-row { flex-direction: column; }
}
</style>
