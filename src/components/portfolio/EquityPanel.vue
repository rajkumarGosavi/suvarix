<script setup lang="ts">
import { ref, computed } from "vue";
import { useConfirm } from "primevue/useconfirm";
import { usePortfolioStore } from "@/stores/portfolio";
import { useHoldingCrud } from "@/composables/useHoldingCrud";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";

const portfolio = usePortfolioStore();
const confirm = useConfirm();
const { formatINR, formatPercent } = useCurrencyFormat();
const { showDialog, editItem, loading, openAdd, openEdit, close, save, remove } =
    useHoldingCrud("add_equity", "update_equity", "delete_equity", portfolio.fetchEquity.bind(portfolio));

const form = computed(() => editItem.value ?? {
    accountId: null, isin: "", symbol: "", exchange: "NSE", name: "", quantity: 0, avgBuyPrice: 0,
});

// Flat list with computed values per row (needed for the expansion detail table)
const flatHoldings = computed(() => portfolio.equity.map((h: any) => ({
    ...h,
    currentValue: h.quantity * (h.currentPrice ?? h.avgBuyPrice),
    investedValue: h.quantity * h.avgBuyPrice,
    get pnl() { return this.currentValue - this.investedValue; },
    get pnlPct() { return this.investedValue > 0 ? (this.pnl / this.investedValue) * 100 : 0; },
    brokerLabel: h.brokerName ?? "Manual",
})));

// Group by ISIN (fallback to symbol when ISIN missing/empty)
const groupedHoldings = computed(() => {
    const map = new Map<string, any>();
    for (const h of flatHoldings.value) {
        const key = h.isin?.trim() || h.symbol;
        if (!map.has(key)) {
            map.set(key, {
                key,
                isin: h.isin,
                symbol: h.symbol,
                name: h.name,
                exchange: h.exchange,
                totalQty: 0,
                totalInvested: 0,
                totalCurrentValue: 0,
                entries: [] as any[],
            });
        }
        const g = map.get(key)!;
        g.totalQty += h.quantity;
        g.totalInvested += h.investedValue;
        g.totalCurrentValue += h.currentValue;
        g.entries.push(h);
    }
    return [...map.values()].map(g => ({
        ...g,
        weightedAvgBuy: g.totalQty > 0 ? g.totalInvested / g.totalQty : 0,
        pnl: g.totalCurrentValue - g.totalInvested,
        pnlPct: g.totalInvested > 0 ? ((g.totalCurrentValue - g.totalInvested) / g.totalInvested) * 100 : 0,
        multipleAccounts: g.entries.length > 1,
    }));
});

// Track which groups are expanded
const expandedKeys = ref<Set<string>>(new Set());

function toggleExpand(key: string) {
    if (expandedKeys.value.has(key)) expandedKeys.value.delete(key);
    else expandedKeys.value.add(key);
    // trigger reactivity
    expandedKeys.value = new Set(expandedKeys.value);
}

function confirmDelete(item: any) {
    confirm.require({
        message: `Remove ${item.name} (${item.symbol}) from ${item.brokerLabel}?`,
        header: "Delete Holding",
        icon: "pi pi-trash",
        rejectProps: { label: "Cancel", outlined: true },
        acceptProps: { label: "Delete" },
        accept: () => remove(item.id),
    });
}

// Severity for broker chips
const BROKER_SEVERITY: Record<string, string> = {
    zerodha: "success",
    upstox: "info",
    angel_one: "warn",
    groww: "secondary",
    mfcentral: "contrast",
};

function brokerSeverity(brokerName: string): string {
    const key = brokerName.toLowerCase().replace(/\s+/g, "_").replace(/\./g, "");
    return BROKER_SEVERITY[key] ?? "secondary";
}
</script>

<template>
    <div class="panel">
        <div class="panel-toolbar">
            <span class="count">{{ groupedHoldings.length }} stock{{ groupedHoldings.length !== 1 ? 's' : '' }}
                <span v-if="flatHoldings.length !== groupedHoldings.length" class="count-detail">
                    ({{ flatHoldings.length }} lots across {{ new Set(flatHoldings.map((h: any) => h.brokerLabel)).size }} brokers)
                </span>
            </span>
            <Button icon="pi pi-plus" label="Add Equity" size="small" @click="openAdd" />
        </div>

        <DataTable
            :value="groupedHoldings"
            stripedRows
            emptyMessage="No equity holdings. Click Add to get started."
            dataKey="key"
        >
            <!-- Expand toggle — only shown when stock exists in multiple accounts -->
            <Column style="width: 2.5rem; padding-right: 0">
                <template #body="{ data }">
                    <Button
                        v-if="data.multipleAccounts"
                        :icon="expandedKeys.has(data.key) ? 'pi pi-chevron-down' : 'pi pi-chevron-right'"
                        text
                        size="small"
                        style="padding: 0.25rem"
                        @click="toggleExpand(data.key)"
                        v-tooltip.right="`${data.entries.length} brokers — click to expand`"
                    />
                </template>
            </Column>

            <Column field="symbol" header="Symbol" sortable style="min-width: 100px">
                <template #body="{ data }">
                    <span class="symbol">{{ data.symbol }}</span>
                    <span v-if="data.isin" class="isin">{{ data.isin }}</span>
                </template>
            </Column>

            <Column field="name" header="Name" sortable style="min-width: 160px" />

            <Column field="exchange" header="Exch" style="width: 70px" />

            <!-- Broker chips — one per account that holds this stock -->
            <Column header="Broker(s)" style="min-width: 140px">
                <template #body="{ data }">
                    <div class="broker-chips">
                        <Tag
                            v-for="entry in data.entries"
                            :key="entry.id"
                            :value="entry.brokerLabel"
                            :severity="brokerSeverity(entry.brokerLabel)"
                            size="small"
                        />
                    </div>
                </template>
            </Column>

            <Column field="totalQty" header="Total Qty" style="width: 90px">
                <template #body="{ data }">{{ data.totalQty }}</template>
            </Column>

            <Column header="Avg Buy" style="width: 110px">
                <template #body="{ data }">{{ formatINR(data.weightedAvgBuy) }}</template>
            </Column>

            <Column field="totalCurrentValue" header="Current Value" sortable style="width: 130px">
                <template #body="{ data }">{{ formatINR(data.totalCurrentValue) }}</template>
            </Column>

            <Column field="pnl" header="P&amp;L" sortable style="width: 150px">
                <template #body="{ data }">
                    <span :class="data.pnl >= 0 ? 'gain' : 'loss'">
                        {{ formatINR(data.pnl) }}<br />
                        <small>{{ formatPercent(data.pnlPct) }}</small>
                    </span>
                </template>
            </Column>

            <!-- Actions (only available for single-broker holdings; multi-broker managed via expand) -->
            <Column header="Actions" style="width: 80px">
                <template #body="{ data }">
                    <template v-if="!data.multipleAccounts">
                        <Button icon="pi pi-pencil" text size="small" aria-label="Edit holding" @click="openEdit(data.entries[0])" />
                        <Button icon="pi pi-trash" text size="small" aria-label="Delete holding" @click="confirmDelete(data.entries[0])" />
                    </template>
                </template>
            </Column>

            <!-- Expanded per-broker breakdown -->
            <template #expansion v-if="false"><!-- placeholder, using custom row below --></template>
        </DataTable>

        <!-- Per-broker detail rows inserted after each expanded group -->
        <template v-for="g in groupedHoldings" :key="`exp-${g.key}`">
            <div v-if="g.multipleAccounts && expandedKeys.has(g.key)" class="broker-breakdown">
                <table class="breakdown-table">
                    <thead>
                        <tr>
                            <th>Broker</th>
                            <th>Qty</th>
                            <th>Avg Buy</th>
                            <th>Current Value</th>
                            <th>P&amp;L</th>
                            <th></th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr v-for="entry in g.entries" :key="entry.id">
                            <td>
                                <Tag :value="entry.brokerLabel" :severity="brokerSeverity(entry.brokerLabel)" size="small" />
                            </td>
                            <td>{{ entry.quantity }}</td>
                            <td>{{ formatINR(entry.avgBuyPrice) }}</td>
                            <td>{{ formatINR(entry.currentValue) }}</td>
                            <td :class="entry.pnl >= 0 ? 'gain' : 'loss'">
                                {{ formatINR(entry.pnl) }} ({{ formatPercent(entry.pnlPct) }})
                            </td>
                            <td>
                                <Button icon="pi pi-pencil" text size="small" aria-label="Edit entry" @click="openEdit(entry)" />
                                <Button icon="pi pi-trash" text size="small" aria-label="Delete entry" @click="confirmDelete(entry)" />
                            </td>
                        </tr>
                    </tbody>
                </table>
            </div>
        </template>
    </div>

    <Dialog v-model:visible="showDialog" :header="editItem ? 'Edit Equity' : 'Add Equity'" modal style="width:480px">
        <form @submit.prevent="save(form)" class="dialog-form">
            <div class="field-row">
                <div class="field">
                    <label>Symbol *</label>
                    <InputText v-model="form.symbol" placeholder="RELIANCE" class="w-full" required />
                </div>
                <div class="field">
                    <label>Exchange *</label>
                    <Select v-model="form.exchange" :options="['NSE','BSE']" class="w-full" />
                </div>
            </div>
            <div class="field">
                <label>Company Name *</label>
                <InputText v-model="form.name" placeholder="Reliance Industries" class="w-full" required />
            </div>
            <div class="field">
                <label>ISIN</label>
                <InputText v-model="form.isin" placeholder="INE002A01018" class="w-full" />
            </div>
            <div class="field-row">
                <div class="field">
                    <label>Quantity *</label>
                    <InputNumber v-model="form.quantity" :min="0" class="w-full" required />
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
</template>

<style scoped>
.panel-toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
}
.count { font-size: 0.875rem; }
.count-detail { color: var(--p-text-muted-color); font-size: 0.8rem; }

.symbol { font-weight: 600; display: block; }
.isin { font-size: 0.72rem; color: var(--p-text-muted-color); display: block; font-family: monospace; }

.broker-chips { display: flex; flex-wrap: wrap; gap: 0.3rem; }

/* Per-broker breakdown panel */
.broker-breakdown {
    background: var(--p-surface-ground);
    border-left: 3px solid var(--p-primary-color);
    margin: 0 0 0.25rem;
    padding: 0.5rem 1rem 0.5rem 2rem;
    border-radius: 0 0 6px 6px;
    overflow-x: auto;
}

.breakdown-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.83rem;
}

.breakdown-table th {
    text-align: left;
    font-size: 0.75rem;
    font-weight: 600;
    color: var(--p-text-muted-color);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    padding: 0.25rem 0.5rem;
    border-bottom: 1px solid var(--p-content-border-color);
}

.breakdown-table td {
    padding: 0.35rem 0.5rem;
    vertical-align: middle;
}

.dialog-form { display: flex; flex-direction: column; gap: 1rem; padding: 0.5rem 0; }
.field { display: flex; flex-direction: column; gap: 0.4rem; flex: 1; }
.field-row { display: flex; gap: 1rem; }
label { font-size: 0.85rem; font-weight: 500; }
.dialog-footer { display: flex; justify-content: flex-end; gap: 0.75rem; margin-top: 0.5rem; }
.gain { color: var(--p-green-500); }
.loss { color: var(--p-red-400); }
</style>


