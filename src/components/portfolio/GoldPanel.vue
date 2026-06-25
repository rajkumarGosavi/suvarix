<script setup lang="ts">
import { reactive, computed } from "vue";
import { useConfirm } from "primevue/useconfirm";
import { usePortfolioStore } from "@/stores/portfolio";
import { useHoldingCrud } from "@/composables/useHoldingCrud";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";
import { strToDate, dateToStr } from "@/composables/useDateConvert";

const portfolio = usePortfolioStore();
const confirm = useConfirm();
const { formatINR } = useCurrencyFormat();
const crud = useHoldingCrud("add_gold", "update_gold", "delete_gold", portfolio.fetchGold.bind(portfolio));

interface GoldForm {
    goldType: string;
    name: string;
    weightGrams: number | null;
    purity: string;
    units: number | null;
    avgBuyPrice: number;
    accountId: number | null;
    maturityDate: Date | null;
}

const form = reactive<GoldForm>({
    goldType: "physical", name: "", weightGrams: null,
    purity: "24K", units: null, avgBuyPrice: 0,
    accountId: null, maturityDate: null,
});

function resetForm() {
    Object.assign(form, {
        goldType: "physical", name: "", weightGrams: null,
        purity: "24K", units: null, avgBuyPrice: 0,
        accountId: null, maturityDate: null,
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
        goldType: item.goldType, name: item.name ?? "", weightGrams: item.weightGrams,
        purity: item.purity ?? "24K", units: item.units, avgBuyPrice: item.avgBuyPrice,
        accountId: item.accountId, maturityDate: strToDate(item.maturityDate),
    });
    crud.showDialog.value = true;
}

async function submit() {
    await crud.save({ ...form, maturityDate: dateToStr(form.maturityDate) });
}

function confirmDelete(item: any) {
    confirm.require({
        message: `Remove ${item.name ?? item.goldType} gold holding?`,
        header: "Delete Gold",
        icon: "pi pi-trash",
        rejectProps: { label: "Cancel", outlined: true },
        acceptProps: { label: "Delete" },
        accept: () => crud.remove(item.id),
    });
}

const holdings = computed(() => portfolio.gold.map((h: any) => ({
    ...h,
    qty: h.weightGrams ?? h.units ?? 0,
    currentValue: (h.weightGrams ?? h.units ?? 0) * (h.currentPrice ?? h.avgBuyPrice),
    investedValue: (h.weightGrams ?? h.units ?? 0) * h.avgBuyPrice,
    get pnl() { return this.currentValue - this.investedValue; },
})));
</script>

<template>
    <div class="panel">
        <div class="panel-toolbar">
            <span class="count">{{ holdings.length }} holdings</span>
            <Button icon="pi pi-plus" label="Add Gold" size="small" @click="openAdd" />
        </div>

        <DataTable :value="holdings" stripedRows emptyMessage="No gold holdings.">
            <Column field="goldType" header="Type">
                <template #body="{ data }">
                    <Tag :value="data.goldType.toUpperCase()" />
                </template>
            </Column>
            <Column field="name" header="Name" />
            <Column header="Qty / Weight">
                <template #body="{ data }">
                    {{ data.weightGrams ? `${data.weightGrams}g` : `${data.units} units` }}
                </template>
            </Column>
            <Column field="avgBuyPrice" header="Avg Buy">
                <template #body="{ data }">{{ formatINR(data.avgBuyPrice) }}</template>
            </Column>
            <Column field="currentValue" header="Value">
                <template #body="{ data }">{{ formatINR(data.currentValue) }}</template>
            </Column>
            <Column field="pnl" header="P&amp;L">
                <template #body="{ data }">{{ formatINR(data.pnl) }}</template>
            </Column>
            <Column field="maturityDate" header="Maturity" />
            <Column header="" style="width:100px">
                <template #body="{ data }">
                    <Button icon="pi pi-pencil" text size="small" @click="openEdit(data)" />
                    <Button icon="pi pi-trash" text size="small" @click="confirmDelete(data)" />
                </template>
            </Column>
        </DataTable>
    </div>

    <Dialog v-model:visible="crud.showDialog.value" :header="crud.editItem.value ? 'Edit Gold' : 'Add Gold Holding'" modal style="width:480px">
        <form @submit.prevent="submit" class="dialog-form">
            <div class="field-row">
                <div class="field">
                    <label>Gold Type *</label>
                    <Select v-model="form.goldType" :options="['physical','digital','etf','sgb']" class="w-full" />
                </div>
                <div class="field">
                    <label>Name / Label</label>
                    <InputText v-model="form.name" placeholder="e.g. SGB 2024 Tranche II" class="w-full" />
                </div>
            </div>

            <template v-if="form.goldType === 'physical'">
                <div class="field-row">
                    <div class="field">
                        <label>Weight (grams)</label>
                        <InputNumber v-model="form.weightGrams" :min="0" :minFractionDigits="2" class="w-full" />
                    </div>
                    <div class="field">
                        <label>Purity</label>
                        <Select v-model="form.purity" :options="['24K','22K','18K','14K']" class="w-full" />
                    </div>
                </div>
            </template>
            <template v-else>
                <div class="field">
                    <label>Units</label>
                    <InputNumber v-model="form.units" :min="0" :minFractionDigits="3" class="w-full" />
                </div>
            </template>

            <div class="field">
                <label>Avg Buy Price (₹ per gram/unit) *</label>
                <InputNumber v-model="form.avgBuyPrice" :min="0" :minFractionDigits="2" class="w-full" required />
            </div>

            <div class="field" v-if="form.goldType === 'sgb'">
                <label>Maturity Date</label>
                <DatePicker v-model="form.maturityDate" dateFormat="dd/mm/yy" showIcon iconDisplay="input" class="w-full" />
            </div>

            <div class="dialog-footer">
                <Button label="Cancel" outlined @click="crud.close()" />
                <Button type="submit" :label="crud.editItem.value ? 'Update' : 'Add'" :loading="crud.loading.value" />
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
