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
const crud = useHoldingCrud("add_real_estate", "update_real_estate", "delete_real_estate", portfolio.fetchRealEstate.bind(portfolio));

interface REForm {
    propertyName: string;
    propertyType: string;
    location: string;
    purchasePrice: number;
    purchaseDate: Date | null;
    currentValue: number | null;
    rentalIncome: number | null;
    hasMortgage: boolean;
}

const form = reactive<REForm>({
    propertyName: "", propertyType: "residential", location: "",
    purchasePrice: 0, purchaseDate: null, currentValue: null,
    rentalIncome: null, hasMortgage: false,
});

function resetForm() {
    Object.assign(form, {
        propertyName: "", propertyType: "residential", location: "",
        purchasePrice: 0, purchaseDate: null, currentValue: null,
        rentalIncome: null, hasMortgage: false,
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
        propertyName: item.propertyName, propertyType: item.propertyType,
        location: item.location ?? "", purchasePrice: item.purchasePrice,
        purchaseDate: strToDate(item.purchaseDate), currentValue: item.currentValue,
        rentalIncome: item.rentalIncome, hasMortgage: item.hasMortgage,
    });
    crud.showDialog.value = true;
}

async function submit() {
    await crud.save({ ...form, purchaseDate: dateToStr(form.purchaseDate) ?? "" });
}

function confirmDelete(item: any) {
    confirm.require({
        message: `Remove ${item.propertyName}?`,
        header: "Delete Property",
        icon: "pi pi-trash",
        rejectProps: { label: "Cancel", outlined: true },
        acceptProps: { label: "Delete" },
        accept: () => crud.remove(item.id),
    });
}

const PROPERTY_TYPES = ["residential", "commercial", "land", "agricultural"];

function gainPct(item: any) {
    if (!item.currentValue) return null;
    return ((item.currentValue - item.purchasePrice) / item.purchasePrice * 100).toFixed(2);
}
</script>

<template>
    <div class="panel">
        <div class="panel-toolbar">
            <span class="count">{{ portfolio.realEstate.length }} properties</span>
            <Button icon="pi pi-plus" label="Add Property" size="small" @click="openAdd" />
        </div>

        <DataTable :value="portfolio.realEstate" stripedRows emptyMessage="No real estate holdings.">
            <Column field="propertyName" header="Property" sortable />
            <Column field="propertyType" header="Type" />
            <Column field="location" header="Location" />
            <Column field="purchasePrice" header="Purchase Price" sortable>
                <template #body="{ data }">{{ formatINR(data.purchasePrice) }}</template>
            </Column>
            <Column field="currentValue" header="Current Value">
                <template #body="{ data }">
                    {{ data.currentValue ? formatINR(data.currentValue) : '—' }}
                </template>
            </Column>
            <Column header="Gain">
                <template #body="{ data }">
                    <span v-if="gainPct(data) !== null"
                          :class="Number(gainPct(data)) >= 0 ? 'gain' : 'loss'">
                        {{ gainPct(data) }}%
                    </span>
                    <span v-else>—</span>
                </template>
            </Column>
            <Column field="rentalIncome" header="Rental/mo">
                <template #body="{ data }">{{ data.rentalIncome ? formatINR(data.rentalIncome) : '—' }}</template>
            </Column>
            <Column header="Actions" style="width:100px">
                <template #body="{ data }">
                    <Button icon="pi pi-pencil" text size="small" aria-label="Edit" @click="openEdit(data)" />
                    <Button icon="pi pi-trash" text size="small" aria-label="Delete" @click="confirmDelete(data)" />
                </template>
            </Column>
        </DataTable>
    </div>

    <Dialog v-model:visible="crud.showDialog.value" :header="crud.editItem.value ? 'Edit Property' : 'Add Real Estate'" modal style="width:520px">
        <form @submit.prevent="submit" class="dialog-form">
            <div class="field">
                <label>Property Name *</label>
                <InputText v-model="form.propertyName" placeholder="My Apartment, Mumbai" class="w-full" required />
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Property Type *</label>
                    <Select v-model="form.propertyType" :options="PROPERTY_TYPES" class="w-full" />
                </div>
                <div class="field">
                    <label>Location</label>
                    <InputText v-model="form.location" placeholder="Mumbai, Maharashtra" class="w-full" />
                </div>
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Purchase Price (₹) *</label>
                    <InputNumber v-model="form.purchasePrice" :min="0" class="w-full" required />
                </div>
                <div class="field">
                    <label>Purchase Date *</label>
                    <DatePicker v-model="form.purchaseDate" dateFormat="dd/mm/yy" showIcon iconDisplay="input" class="w-full" required />
                </div>
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Current Value (₹)</label>
                    <InputNumber v-model="form.currentValue" :min="0" class="w-full" />
                </div>
                <div class="field">
                    <label>Rental Income / month (₹)</label>
                    <InputNumber v-model="form.rentalIncome" :min="0" class="w-full" />
                </div>
            </div>
            <div class="field-row field--check-row">
                <Checkbox v-model="form.hasMortgage" binary inputId="hasMortgage" />
                <label for="hasMortgage">Has active mortgage / home loan</label>
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

