<script setup lang="ts">
import { computed } from "vue";
import { useConfirm } from "primevue/useconfirm";
import { usePortfolioStore } from "@/stores/portfolio";
import { useHoldingCrud } from "@/composables/useHoldingCrud";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";

const portfolio = usePortfolioStore();
const confirm = useConfirm();
const { formatINR, formatPercent } = useCurrencyFormat();
const { showDialog, editItem, loading, openAdd, openEdit, close, save, remove } =
    useHoldingCrud("add_crypto", "update_crypto", "delete_crypto", portfolio.fetchCrypto.bind(portfolio));

const form = computed(() => editItem.value ?? {
    accountId: null, exchangeName: "", coinSymbol: "", quantity: 0, avgBuyPrice: 0,
});

const holdings = computed(() => portfolio.crypto.map((h: any) => ({
    ...h,
    currentValue: h.quantity * (h.currentPrice ?? h.avgBuyPrice),
    investedValue: h.quantity * h.avgBuyPrice,
    get pnl() { return this.currentValue - this.investedValue; },
    get pnlPct() { return this.investedValue > 0 ? (this.pnl / this.investedValue) * 100 : 0; },
})));

function confirmDelete(item: any) {
    confirm.require({
        message: `Remove ${item.coinSymbol} on ${item.exchangeName}?`,
        header: "Delete Crypto",
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
            <span class="count">{{ holdings.length }} holdings</span>
            <Button icon="pi pi-plus" label="Add Crypto" size="small" @click="openAdd" />
        </div>

        <DataTable :value="holdings" stripedRows emptyMessage="No crypto holdings.">
            <Column field="coinSymbol" header="Coin" sortable />
            <Column field="exchangeName" header="Exchange" />
            <Column field="quantity" header="Quantity">
                <template #body="{ data }">{{ data.quantity }}</template>
            </Column>
            <Column field="avgBuyPrice" header="Avg Buy (₹)">
                <template #body="{ data }">{{ formatINR(data.avgBuyPrice) }}</template>
            </Column>
            <Column field="currentValue" header="Value (₹)" sortable>
                <template #body="{ data }">{{ formatINR(data.currentValue) }}</template>
            </Column>
            <Column field="pnl" header="P&amp;L" sortable>
                <template #body="{ data }">
                    <span :class="data.pnl >= 0 ? 'gain' : 'loss'">
                        {{ formatINR(data.pnl) }} ({{ formatPercent(data.pnlPct) }})
                    </span>
                </template>
            </Column>
            <Column header="" style="width:100px">
                <template #body="{ data }">
                    <Button icon="pi pi-pencil" text size="small" @click="openEdit(data)" />
                    <Button icon="pi pi-trash" text size="small" @click="confirmDelete(data)" />
                </template>
            </Column>
        </DataTable>
    </div>

    <Dialog v-model:visible="showDialog" :header="editItem ? 'Edit Crypto' : 'Add Crypto Holding'" modal style="width:480px">
        <form @submit.prevent="save(form)" class="dialog-form">
            <div class="field-row">
                <div class="field">
                    <label>Coin Symbol *</label>
                    <InputText v-model="form.coinSymbol" placeholder="BTC" class="w-full" required />
                </div>
                <div class="field">
                    <label>Exchange *</label>
                    <InputText v-model="form.exchangeName" placeholder="WazirX / CoinDCX" class="w-full" required />
                </div>
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Quantity *</label>
                    <InputNumber v-model="form.quantity" :min="0" :minFractionDigits="6" class="w-full" required />
                </div>
                <div class="field">
                    <label>Avg Buy Price (₹) *</label>
                    <InputNumber v-model="form.avgBuyPrice" :min="0" :minFractionDigits="2" class="w-full" required />
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
