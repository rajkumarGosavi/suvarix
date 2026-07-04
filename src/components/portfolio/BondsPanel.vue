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
const crud = useHoldingCrud("add_bond", "update_bond", "delete_bond", portfolio.fetchBonds.bind(portfolio));

interface BondForm {
    accountId: number | null;
    isin: string;
    issuerName: string;
    bondType: string;
    faceValue: number;
    quantity: number;
    purchasePrice: number;
    currentPrice: number | null;
    couponRate: number;
    couponFrequency: string;
    purchaseDate: Date | null;
    maturityDate: Date | null;
    creditRating: string;
}

const form = reactive<BondForm>({
    accountId: null,
    isin: "",
    issuerName: "",
    bondType: "corporate",
    faceValue: 1000,
    quantity: 1,
    purchasePrice: 1000,
    currentPrice: null,
    couponRate: 0,
    couponFrequency: "semi_annual",
    purchaseDate: null,
    maturityDate: null,
    creditRating: "",
});

function resetForm() {
    Object.assign(form, {
        accountId: null, isin: "", issuerName: "", bondType: "corporate",
        faceValue: 1000, quantity: 1, purchasePrice: 1000, currentPrice: null,
        couponRate: 0, couponFrequency: "semi_annual",
        purchaseDate: null, maturityDate: null, creditRating: "",
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
        accountId: item.accountId,
        isin: item.isin ?? "",
        issuerName: item.issuerName,
        bondType: item.bondType,
        faceValue: item.faceValue,
        quantity: item.quantity,
        purchasePrice: item.purchasePrice,
        currentPrice: item.currentPrice ?? null,
        couponRate: item.couponRate,
        couponFrequency: item.couponFrequency,
        purchaseDate: strToDate(item.purchaseDate),
        maturityDate: item.maturityDate ? strToDate(item.maturityDate) : null,
        creditRating: item.creditRating ?? "",
    });
    crud.showDialog.value = true;
}

async function submit() {
    await crud.save({
        accountId: form.accountId,
        isin: form.isin || null,
        issuerName: form.issuerName,
        bondType: form.bondType,
        faceValue: form.faceValue,
        quantity: form.quantity,
        purchasePrice: form.purchasePrice,
        currentPrice: form.currentPrice,
        couponRate: form.couponRate,
        couponFrequency: form.couponFrequency,
        purchaseDate: dateToStr(form.purchaseDate) ?? "",
        maturityDate: dateToStr(form.maturityDate) ?? null,
        creditRating: form.creditRating || null,
    });
}

function confirmDelete(item: any) {
    confirm.require({
        message: `Remove bond: ${item.issuerName}?`,
        header: "Delete Bond",
        icon: "pi pi-trash",
        rejectProps: { label: "Cancel", outlined: true },
        acceptProps: { label: "Delete" },
        accept: () => crud.remove(item.id),
    });
}

const BOND_TYPES = [
    { label: "Corporate", value: "corporate" },
    { label: "Government", value: "government" },
    { label: "Tax-Free", value: "tax_free" },
    { label: "SGB", value: "sgb" },
    { label: "NCD", value: "ncd" },
    { label: "Treasury Bill", value: "treasury_bill" },
];

const COUPON_FREQUENCIES = [
    { label: "Semi-Annual", value: "semi_annual" },
    { label: "Annual", value: "annual" },
    { label: "Quarterly", value: "quarterly" },
    { label: "Monthly", value: "monthly" },
    { label: "Zero Coupon", value: "zero_coupon" },
];

const BOND_TYPE_LABELS: Record<string, string> = {
    corporate: "Corporate",
    government: "Government",
    tax_free: "Tax-Free",
    sgb: "SGB",
    ncd: "NCD",
    treasury_bill: "T-Bill",
};

function bondTypeSeverity(type: string): string {
    const map: Record<string, string> = {
        government: "success",
        tax_free: "info",
        sgb: "warn",
        corporate: "secondary",
        ncd: "secondary",
        treasury_bill: "contrast",
    };
    return map[type] ?? "secondary";
}

function daysToMaturity(maturityDate: string | null) {
    if (!maturityDate) return "—";
    const diff = new Date(maturityDate).getTime() - Date.now();
    const days = Math.ceil(diff / 86400000);
    if (days < 0) return "Matured";
    if (days < 30) return `${days}d`;
    if (days < 365) return `${Math.floor(days / 30)}mo`;
    return `${(days / 365).toFixed(1)}yr`;
}

const totalInvestment = computed(() =>
    portfolio.bonds.reduce((s: number, b: any) => s + b.quantity * b.purchasePrice, 0)
);
const totalCurrentValue = computed(() =>
    portfolio.bonds.reduce((s: number, b: any) =>
        s + b.quantity * (b.currentPrice ?? b.purchasePrice), 0)
);
const totalPnl = computed(() => totalCurrentValue.value - totalInvestment.value);
</script>

<template>
    <div class="panel">
        <div class="panel-toolbar">
            <span class="count">{{ portfolio.bonds.length }} bond{{ portfolio.bonds.length !== 1 ? "s" : "" }}</span>
            <Button icon="pi pi-plus" label="Add Bond" size="small" @click="openAdd" />
        </div>

        <div class="summary-row" v-if="portfolio.bonds.length > 0">
            <div class="summary-item">
                <span class="summary-label">Invested</span>
                <span class="summary-value">{{ formatINR(totalInvestment) }}</span>
            </div>
            <div class="summary-item">
                <span class="summary-label">Current Value</span>
                <span class="summary-value">{{ formatINR(totalCurrentValue) }}</span>
            </div>
            <div class="summary-item">
                <span class="summary-label">P&amp;L</span>
                <span class="summary-value" :class="totalPnl >= 0 ? 'pnl-pos' : 'pnl-neg'">
                    {{ totalPnl >= 0 ? "+" : "" }}{{ formatINR(totalPnl) }}
                </span>
            </div>
        </div>

        <DataTable :value="portfolio.bonds" stripedRows emptyMessage="No bonds added.">
            <Column field="issuerName" header="Issuer" sortable />
            <Column header="Type" style="width:110px">
                <template #body="{ data }">
                    <Tag :value="BOND_TYPE_LABELS[data.bondType] ?? data.bondType"
                         :severity="bondTypeSeverity(data.bondType)" />
                </template>
            </Column>
            <Column field="couponRate" header="Coupon" style="width:90px">
                <template #body="{ data }">{{ data.couponRate }}%</template>
            </Column>
            <Column field="quantity" header="Qty" style="width:70px" />
            <Column header="Invested" sortable>
                <template #body="{ data }">{{ formatINR(data.quantity * data.purchasePrice) }}</template>
            </Column>
            <Column header="Current Value">
                <template #body="{ data }">
                    {{ formatINR(data.quantity * (data.currentPrice ?? data.purchasePrice)) }}
                </template>
            </Column>
            <Column header="P&amp;L">
                <template #body="{ data }">
                    <span :class="(data.currentPrice ?? data.purchasePrice) >= data.purchasePrice ? 'pnl-pos' : 'pnl-neg'">
                        {{ formatINR(data.quantity * ((data.currentPrice ?? data.purchasePrice) - data.purchasePrice)) }}
                    </span>
                </template>
            </Column>
            <Column header="Matures In" style="width:110px">
                <template #body="{ data }">
                    <Tag :value="daysToMaturity(data.maturityDate)" />
                </template>
            </Column>
            <Column field="creditRating" header="Rating" style="width:80px">
                <template #body="{ data }">{{ data.creditRating ?? "—" }}</template>
            </Column>
            <Column header="Actions" style="width:100px">
                <template #body="{ data }">
                    <Button icon="pi pi-pencil" text size="small" aria-label="Edit" @click="openEdit(data)" />
                    <Button icon="pi pi-trash" text size="small" aria-label="Delete" @click="confirmDelete(data)" />
                </template>
            </Column>
        </DataTable>
    </div>

    <Dialog v-model:visible="crud.showDialog.value"
            :header="crud.editItem.value ? 'Edit Bond' : 'Add Bond'"
            modal style="width:560px">
        <form @submit.prevent="submit" class="dialog-form">
            <div class="field-row">
                <div class="field">
                    <label>Issuer Name *</label>
                    <InputText v-model="form.issuerName" placeholder="HDFC Ltd." class="w-full" required />
                </div>
                <div class="field">
                    <label>Bond Type *</label>
                    <Select v-model="form.bondType" :options="BOND_TYPES"
                            optionLabel="label" optionValue="value" class="w-full" />
                </div>
            </div>
            <div class="field-row">
                <div class="field">
                    <label>ISIN</label>
                    <InputText v-model="form.isin" placeholder="INE001A07QQ5" class="w-full" />
                </div>
                <div class="field">
                    <label>Credit Rating</label>
                    <InputText v-model="form.creditRating" placeholder="AAA, AA+, A…" class="w-full" />
                </div>
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Face Value (₹) *</label>
                    <InputNumber v-model="form.faceValue" :min="1" class="w-full" required />
                </div>
                <div class="field">
                    <label>Quantity *</label>
                    <InputNumber v-model="form.quantity" :min="0.001" :minFractionDigits="0" :maxFractionDigits="3" class="w-full" required />
                </div>
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Purchase Price (₹) *</label>
                    <InputNumber v-model="form.purchasePrice" :min="0" :minFractionDigits="2" class="w-full" required />
                </div>
                <div class="field">
                    <label>Current Price (₹)</label>
                    <InputNumber v-model="form.currentPrice" :min="0" :minFractionDigits="2" class="w-full" />
                </div>
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Coupon Rate (%)</label>
                    <InputNumber v-model="form.couponRate" :min="0" :max="30" :minFractionDigits="2" class="w-full" />
                </div>
                <div class="field">
                    <label>Coupon Frequency</label>
                    <Select v-model="form.couponFrequency" :options="COUPON_FREQUENCIES"
                            optionLabel="label" optionValue="value" class="w-full" />
                </div>
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Purchase Date *</label>
                    <DatePicker v-model="form.purchaseDate" dateFormat="dd/mm/yy"
                                showIcon iconDisplay="input" class="w-full" required />
                </div>
                <div class="field">
                    <label>Maturity Date</label>
                    <DatePicker v-model="form.maturityDate" dateFormat="dd/mm/yy"
                                showIcon iconDisplay="input" class="w-full" />
                </div>
            </div>
            <div class="dialog-footer">
                <Button label="Cancel" outlined @click="crud.close()" />
                <Button type="submit" :label="crud.editItem.value ? 'Update' : 'Add'"
                        :loading="crud.loading.value" />
            </div>
        </form>
    </Dialog>
</template>

<style scoped>
.panel-toolbar { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; }
.count { font-size: 0.875rem; }
.summary-row {
    display: flex;
    gap: 2rem;
    padding: 0.75rem 1rem;
    background: var(--p-surface-ground);
    border-radius: 8px;
    margin-bottom: 1rem;
}
.summary-item { display: flex; flex-direction: column; gap: 0.2rem; }
.summary-label { font-size: 0.75rem; color: var(--p-text-muted-color); }
.summary-value { font-size: 1rem; font-weight: 600; }
.dialog-form { display: flex; flex-direction: column; gap: 1rem; padding: 0.5rem 0; }
.field { display: flex; flex-direction: column; gap: 0.4rem; flex: 1; }
.field-row { display: flex; gap: 1rem; }
label { font-size: 0.85rem; font-weight: 500; }
.dialog-footer { display: flex; justify-content: flex-end; gap: 0.75rem; margin-top: 0.5rem; }
.pnl-pos { color: var(--p-green-500); }
.pnl-neg { color: var(--p-red-500); }
</style>

