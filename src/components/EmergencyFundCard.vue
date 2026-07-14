<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useFinancialHealthStore } from "@/stores/financialHealth";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";

const store = useFinancialHealthStore();
const { formatCompact, formatINR } = useCurrencyFormat();

const ef = computed(() => store.emergencyFund);

const editing = ref(false);
const draftMonths = ref(6);

watch(
    () => store.emergencyFund?.targetMonths,
    (m) => {
        if (typeof m === "number") draftMonths.value = m;
    },
    { immediate: true },
);

// status → colour (theme-aware PrimeVue vars with hex fallback).
const accent = computed(() => {
    switch (ef.value?.status) {
        case "funded":
            return "var(--p-green-500, #22c55e)";
        case "on_track":
            return "var(--p-amber-500, #f59e0b)";
        default:
            return "var(--p-red-500, #ef4444)";
    }
});

const statusLabel = computed(() => {
    switch (ef.value?.status) {
        case "funded":
            return "Fully funded 🎉";
        case "on_track":
            return "On track";
        default:
            return "Underfunded";
    }
});

async function saveTarget() {
    await store.setEmergencyTarget(draftMonths.value);
    editing.value = false;
}
</script>

<template>
    <div class="ef card" v-if="ef && ef.monthlyExpense > 0">
        <div class="ef-header">
            <span class="ef-title">Emergency Fund</span>
            <span class="ef-status" :style="{ color: accent }">{{ statusLabel }}</span>
        </div>

        <div class="ef-amounts">
            <span class="ef-current">{{ formatCompact(ef.liquidAssets) }}</span>
            <span class="ef-target">/ {{ formatCompact(ef.targetAmount) }} goal</span>
        </div>

        <div class="ef-bar">
            <div class="ef-bar-fill" :style="{ width: `${ef.coveragePct}%`, background: accent }" />
        </div>

        <div class="ef-meta">
            <span>{{ ef.monthsCovered }} of {{ ef.targetMonths }} months covered</span>
            <span v-if="ef.shortfall > 0" class="ef-short">{{ formatCompact(ef.shortfall) }} to go</span>
        </div>

        <div class="ef-target-row">
            <template v-if="!editing">
                <span class="ef-target-note">Goal: {{ ef.targetMonths }} months of expenses ({{ formatINR(ef.monthlyExpense) }}/mo)</span>
                <Button label="Change goal" text size="small" class="ef-edit" @click="editing = true" />
            </template>
            <template v-else>
                <label for="ef-months">Months of expenses</label>
                <InputNumber
                    input-id="ef-months"
                    v-model="draftMonths"
                    :min="1"
                    :max="24"
                    showButtons
                    size="small"
                    class="ef-input"
                />
                <Button label="Save" size="small" @click="saveTarget" />
                <Button label="Cancel" text size="small" severity="secondary" @click="editing = false" />
            </template>
        </div>
    </div>
</template>

<style scoped>
.ef {
    margin-bottom: 1.5rem;
}

.ef-header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin-bottom: 0.75rem;
}

.ef-title {
    font-size: 1rem;
    font-weight: 600;
}

.ef-status {
    font-size: 0.8rem;
    font-weight: 700;
}

.ef-amounts {
    display: flex;
    align-items: baseline;
    gap: 0.4rem;
    margin-bottom: 0.6rem;
}

.ef-current {
    font-size: 1.6rem;
    font-weight: 800;
    letter-spacing: -0.01em;
}

.ef-target {
    font-size: 0.85rem;
    color: var(--p-text-muted-color);
}

.ef-bar {
    width: 100%;
    height: 8px;
    border-radius: 4px;
    background: var(--p-content-border-color, #e5e7eb);
    overflow: hidden;
    margin-bottom: 0.5rem;
}

.ef-bar-fill {
    height: 100%;
    border-radius: 4px;
    transition: width 0.4s ease;
}

.ef-meta {
    display: flex;
    justify-content: space-between;
    font-size: 0.78rem;
    color: var(--p-text-muted-color);
    margin-bottom: 0.75rem;
}

.ef-short {
    font-weight: 600;
}

.ef-target-row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-wrap: wrap;
    padding-top: 0.75rem;
    border-top: 1px solid var(--p-content-border-color);
}

.ef-target-note {
    font-size: 0.8rem;
    color: var(--p-text-muted-color);
    flex: 1;
    min-width: 0;
}

.ef-target-row label {
    font-size: 0.82rem;
    font-weight: 500;
}

.ef-edit {
    padding-left: 0;
    padding-right: 0;
}

@media (max-width: 639px) {
    .ef-target-note {
        flex-basis: 100%;
    }
}
</style>
