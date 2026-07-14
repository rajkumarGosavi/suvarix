<script setup lang="ts">
import { onMounted, ref, computed, watch } from "vue";
import { useLiabilitiesStore, type DebtPlan } from "@/stores/liabilities";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";

const store = useLiabilitiesStore();
const { formatINR, formatCompact } = useCurrencyFormat();

const strategy = ref<"avalanche" | "snowball">("avalanche");
const extraMonthly = ref(0);
const plan = ref<DebtPlan | null>(null);
const loading = ref(false);

const strategyOptions = [
    { label: "Avalanche", value: "avalanche" },
    { label: "Snowball", value: "snowball" },
];

const strategyHint = computed(() =>
    strategy.value === "avalanche"
        ? "Targets the highest interest rate first — least total interest."
        : "Targets the smallest balance first — fastest wins for motivation.",
);

async function refresh() {
    loading.value = true;
    try {
        plan.value = await store.getPayoffPlan(strategy.value, Math.max(0, extraMonthly.value || 0));
    } catch {
        plan.value = null;
    } finally {
        loading.value = false;
    }
}

// Debounce the extra-payment input; react immediately to strategy changes.
let t: ReturnType<typeof setTimeout> | undefined;
watch([strategy, extraMonthly], () => {
    clearTimeout(t);
    t = setTimeout(refresh, 250);
});

onMounted(refresh);

const hasDebt = computed(() => (plan.value?.steps.length ?? 0) > 0);

function monthsLabel(m: number): string {
    if (m <= 0) return "—";
    const y = Math.floor(m / 12);
    const mo = m % 12;
    if (y === 0) return `${mo} mo`;
    if (mo === 0) return `${y} yr`;
    return `${y} yr ${mo} mo`;
}

const debtFreeDate = computed(() => {
    if (!plan.value || plan.value.neverClears || plan.value.monthsToDebtFree <= 0) return null;
    const d = new Date();
    d.setMonth(d.getMonth() + plan.value.monthsToDebtFree);
    return d.toLocaleDateString("en-IN", { month: "short", year: "numeric" });
});

// Ordered by actual payoff month so the list reads as the plan's sequence.
const orderedSteps = computed(() =>
    [...(plan.value?.steps ?? [])].sort((a, b) => {
        const am = a.payoffMonth || Infinity;
        const bm = b.payoffMonth || Infinity;
        return am - bm;
    }),
);
</script>

<template>
    <div class="payoff card" v-if="hasDebt || loading">
        <div class="po-header">
            <span class="po-title">Debt Payoff Planner</span>
            <SelectButton
                v-model="strategy"
                :options="strategyOptions"
                option-label="label"
                option-value="value"
                :allow-empty="false"
                size="small"
            />
        </div>
        <p class="po-hint">{{ strategyHint }}</p>

        <div class="po-extra">
            <label for="po-extra-input">Extra per month</label>
            <InputNumber
                input-id="po-extra-input"
                v-model="extraMonthly"
                mode="currency"
                currency="INR"
                :min="0"
                :step="1000"
                showButtons
                size="small"
            />
        </div>

        <template v-if="plan && hasDebt">
            <div v-if="plan.neverClears" class="po-warn">
                ⚠️ At the current budget ({{ formatINR(plan.monthlyBudget) }}/mo) the interest
                outpaces payments — the debt never clears. Add an extra payment above.
            </div>

            <template v-else>
                <div class="po-stats">
                    <div class="po-stat">
                        <span class="po-stat-value">{{ monthsLabel(plan.monthsToDebtFree) }}</span>
                        <span class="po-stat-label">to debt-free<span v-if="debtFreeDate"> · {{ debtFreeDate }}</span></span>
                    </div>
                    <div class="po-stat">
                        <span class="po-stat-value">{{ formatCompact(plan.totalInterest) }}</span>
                        <span class="po-stat-label">total interest</span>
                    </div>
                    <div class="po-stat po-stat--good" v-if="plan.interestSaved > 0">
                        <span class="po-stat-value">−{{ formatCompact(plan.interestSaved) }}</span>
                        <span class="po-stat-label">interest saved<span v-if="plan.monthsSaved > 0"> · {{ monthsLabel(plan.monthsSaved) }} sooner</span></span>
                    </div>
                </div>

                <div class="po-order">
                    <div class="po-order-label">Payoff order</div>
                    <div v-for="(s, i) in orderedSteps" :key="s.kind + s.id" class="po-row">
                        <span class="po-rank">{{ i + 1 }}</span>
                        <div class="po-row-main">
                            <span class="po-name">{{ s.name }}</span>
                            <span class="po-meta">{{ formatINR(s.balance) }} · {{ s.annualRatePct }}%</span>
                        </div>
                        <span class="po-when">{{ s.payoffMonth ? monthsLabel(s.payoffMonth) : "—" }}</span>
                    </div>
                </div>
            </template>
        </template>
    </div>
</template>

<style scoped>
.payoff {
    margin-bottom: 1.5rem;
}

.po-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
    margin-bottom: 0.4rem;
}

.po-title {
    font-size: 1rem;
    font-weight: 600;
}

.po-hint {
    font-size: 0.78rem;
    color: var(--p-text-muted-color);
    margin: 0 0 0.9rem;
}

.po-extra {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-bottom: 1rem;
    flex-wrap: wrap;
}

.po-extra label {
    font-size: 0.85rem;
    font-weight: 500;
}

.po-warn {
    font-size: 0.85rem;
    padding: 0.75rem 0.9rem;
    border-radius: 8px;
    background: color-mix(in srgb, var(--p-red-500, #ef4444) 12%, transparent);
    color: var(--p-red-600, #dc2626);
}

.po-stats {
    display: flex;
    gap: 1.5rem;
    flex-wrap: wrap;
    padding: 0.9rem 0;
    border-top: 1px solid var(--p-content-border-color);
    border-bottom: 1px solid var(--p-content-border-color);
    margin-bottom: 1rem;
}

.po-stat {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
}

.po-stat-value {
    font-size: 1.3rem;
    font-weight: 800;
    letter-spacing: -0.01em;
}

.po-stat-label {
    font-size: 0.72rem;
    color: var(--p-text-muted-color);
}

.po-stat--good .po-stat-value {
    color: var(--p-green-500, #22c55e);
}

.po-order-label {
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--p-text-muted-color);
    margin-bottom: 0.5rem;
}

.po-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.5rem 0;
    border-bottom: 1px solid var(--p-content-border-color);
}

.po-row:last-child {
    border-bottom: none;
}

.po-rank {
    width: 22px;
    height: 22px;
    flex-shrink: 0;
    border-radius: 50%;
    display: grid;
    place-items: center;
    font-size: 0.72rem;
    font-weight: 700;
    background: var(--p-primary-color);
    color: var(--p-primary-contrast-color, #fff);
}

.po-row-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
}

.po-name {
    font-size: 0.88rem;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.po-meta {
    font-size: 0.76rem;
    color: var(--p-text-muted-color);
}

.po-when {
    font-size: 0.82rem;
    font-weight: 600;
    flex-shrink: 0;
}

@media (max-width: 639px) {
    .po-stats {
        gap: 1rem;
    }
}
</style>
