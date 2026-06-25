<script setup lang="ts">
import { ref, reactive, computed, onMounted } from "vue";
import { useConfirm } from "primevue/useconfirm";
import { useGoalsStore, type GoalPayload } from "@/stores/goals";
import { usePortfolioStore } from "@/stores/portfolio";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";
import { strToDate, dateToStr } from "@/composables/useDateConvert";
import { useAnalytics } from "@/composables/useAnalytics";

const goalsStore = useGoalsStore();
const portfolio = usePortfolioStore();
const confirm = useConfirm();
const { formatCompact } = useCurrencyFormat();
const { track } = useAnalytics();

// ─── dialog state ─────────────────────────────────────────────

const showDialog = ref(false);
const editId = ref<number | null>(null);
const saving = ref(false);

interface GoalForm {
    name: string;
    category: string;
    targetAmount: number;
    targetDate: Date | null;
    notes: string;
}

const form = reactive<GoalForm>({
    name: "",
    category: "other",
    targetAmount: 0,
    targetDate: null,
    notes: "",
});

function resetForm() {
    form.name = "";
    form.category = "other";
    form.targetAmount = 0;
    form.targetDate = null;
    form.notes = "";
}

function openAdd() {
    editId.value = null;
    resetForm();
    showDialog.value = true;
}

function openEdit(goal: any) {
    editId.value = goal.id;
    form.name = goal.name;
    form.category = goal.category;
    form.targetAmount = goal.targetAmount;
    form.targetDate = strToDate(goal.targetDate);
    form.notes = goal.notes ?? "";
    showDialog.value = true;
}

async function saveGoal() {
    if (!form.targetDate) return;
    const payload: GoalPayload = {
        name: form.name,
        category: form.category,
        targetAmount: form.targetAmount,
        targetDate: dateToStr(form.targetDate)!,
        notes: form.notes.trim() || null,
    };
    saving.value = true;
    try {
        if (editId.value !== null) {
            await goalsStore.updateGoal(editId.value, payload);
        } else {
            await goalsStore.addGoal(payload);
        }
        track("goal_saved", { category: payload.category });
        showDialog.value = false;
    } finally {
        saving.value = false;
    }
}

function confirmDelete(goal: any) {
    confirm.require({
        message: `Delete "${goal.name}"?`,
        header: "Delete Goal",
        icon: "pi pi-exclamation-triangle",
        rejectProps: { label: "Cancel", severity: "secondary", outlined: true },
        acceptProps: { label: "Delete", severity: "danger" },
        accept: () => goalsStore.deleteGoal(goal.id),
    });
}

// ─── computed per-goal metrics ─────────────────────────────────

const totalAssets = computed(() => portfolio.netWorth?.totalAssets ?? 0);

const enrichedGoals = computed(() =>
    goalsStore.goals.map((g) => {
        const progress = Math.min((totalAssets.value / g.targetAmount) * 100, 100);
        const today = new Date();
        const target = strToDate(g.targetDate)!;
        const diffMs = target.getTime() - today.getTime();
        const monthsLeft = Math.ceil(diffMs / (1000 * 60 * 60 * 24 * 30.44));
        const achieved = totalAssets.value >= g.targetAmount;
        return { ...g, progress, monthsLeft, achieved };
    }),
);

// ─── category helpers ──────────────────────────────────────────

const CATEGORIES = [
    { value: "home",       label: "Home",         icon: "pi pi-home" },
    { value: "vehicle",    label: "Vehicle",       icon: "pi pi-car" },
    { value: "education",  label: "Education",     icon: "pi pi-book" },
    { value: "retirement", label: "Retirement",    icon: "pi pi-sun" },
    { value: "travel",     label: "Travel",        icon: "pi pi-globe" },
    { value: "emergency",  label: "Emergency Fund", icon: "pi pi-shield" },
    { value: "other",      label: "Other",         icon: "pi pi-star" },
];

function categoryIcon(cat: string) {
    return CATEGORIES.find((c) => c.value === cat)?.icon ?? "pi pi-star";
}

function categoryLabel(cat: string) {
    return CATEGORIES.find((c) => c.value === cat)?.label ?? cat;
}

function monthsLeftLabel(months: number) {
    if (months <= 0) return "Overdue";
    if (months < 12) return `${months} month${months !== 1 ? "s" : ""} left`;
    const y = Math.floor(months / 12);
    const m = months % 12;
    return m === 0 ? `${y} yr${y !== 1 ? "s" : ""} left` : `${y}y ${m}m left`;
}

onMounted(() => {
    goalsStore.fetchGoals();
    portfolio.fetchNetWorth();
});
</script>

<template>
    <div class="goals-view">
        <div class="goals-header">
            <div>
                <h2 class="goals-title">Goals</h2>
                <p class="goals-subtitle">
                    Progress based on your total portfolio value
                    <strong>{{ formatCompact(totalAssets) }}</strong>
                </p>
            </div>
            <Button icon="pi pi-plus" label="Add Goal" @click="openAdd" />
        </div>

        <!-- Empty state -->
        <div v-if="!goalsStore.loading && enrichedGoals.length === 0" class="goals-empty">
            <i class="pi pi-flag goals-empty-icon" />
            <p>No goals yet. Set a target to track your progress.</p>
            <Button label="Add Goal" icon="pi pi-plus" outlined @click="openAdd" />
        </div>

        <!-- Cards grid -->
        <div class="goals-grid">
            <div
                v-for="g in enrichedGoals"
                :key="g.id"
                class="goal-card"
                :class="{ 'goal-card--achieved': g.achieved }"
            >
                <div class="goal-card-header">
                    <div class="goal-card-title">
                        <i :class="categoryIcon(g.category)" class="goal-cat-icon" />
                        <span class="goal-name">{{ g.name }}</span>
                    </div>
                    <div class="goal-card-actions">
                        <Button icon="pi pi-pencil" text size="small" @click="openEdit(g)" />
                        <Button icon="pi pi-trash" text size="small" severity="danger" @click="confirmDelete(g)" />
                    </div>
                </div>

                <!-- Progress bar -->
                <div class="goal-progress-wrap">
                    <ProgressBar
                        :value="g.progress"
                        :pt="{ value: { class: g.achieved ? 'goal-bar-achieved' : 'goal-bar-active' } }"
                        class="goal-bar"
                    />
                    <span class="goal-pct" :class="{ 'goal-pct--achieved': g.achieved }">
                        {{ g.achieved ? "✓" : g.progress.toFixed(1) + "%" }}
                    </span>
                </div>

                <!-- Amounts row -->
                <div class="goal-amounts">
                    <div class="goal-amount-item">
                        <span class="goal-amount-label">Current</span>
                        <span class="goal-amount-value">{{ formatCompact(totalAssets) }}</span>
                    </div>
                    <i class="pi pi-arrow-right goal-arrow" />
                    <div class="goal-amount-item">
                        <span class="goal-amount-label">Target</span>
                        <span class="goal-amount-value goal-amount-target">{{ formatCompact(g.targetAmount) }}</span>
                    </div>
                </div>

                <!-- Footer row -->
                <div class="goal-footer">
                    <Tag
                        v-if="g.achieved"
                        value="Achieved"
                        severity="success"
                        icon="pi pi-check"
                    />
                    <template v-else>
                        <span class="goal-date">
                            {{ new Date(g.targetDate + "T00:00:00").toLocaleDateString("en-IN", { month: "short", year: "numeric" }) }}
                        </span>
                        <Tag
                            :value="monthsLeftLabel(g.monthsLeft)"
                            :severity="g.monthsLeft < 6 ? 'warn' : 'secondary'"
                            size="small"
                        />
                    </template>
                    <span class="goal-cat-badge">{{ categoryLabel(g.category) }}</span>
                </div>
            </div>
        </div>

        <!-- Add / Edit dialog -->
        <Dialog
            v-model:visible="showDialog"
            :header="editId !== null ? 'Edit Goal' : 'New Goal'"
            modal
            :style="{ width: '480px', maxWidth: '95vw' }"
        >
            <form @submit.prevent="saveGoal" class="goal-form">
                <div class="field">
                    <label>Goal Name *</label>
                    <InputText v-model="form.name" placeholder="e.g. Buy a House" class="w-full" required />
                </div>
                <div class="field">
                    <label>Category</label>
                    <Select
                        v-model="form.category"
                        :options="CATEGORIES"
                        optionLabel="label"
                        optionValue="value"
                        class="w-full"
                    >
                        <template #option="{ option }">
                            <div class="goal-cat-option">
                                <i :class="option.icon" />
                                <span>{{ option.label }}</span>
                            </div>
                        </template>
                    </Select>
                </div>
                <div class="field-row">
                    <div class="field">
                        <label>Target Amount (₹) *</label>
                        <InputNumber
                            v-model="form.targetAmount"
                            :min="1"
                            :minFractionDigits="0"
                            :maxFractionDigits="0"
                            class="w-full"
                            required
                        />
                    </div>
                    <div class="field">
                        <label>Target Date *</label>
                        <DatePicker
                            v-model="form.targetDate"
                            dateFormat="dd/mm/yy"
                            showIcon
                            iconDisplay="input"
                            class="w-full"
                            required
                        />
                    </div>
                </div>
                <div class="field">
                    <label>Notes</label>
                    <Textarea v-model="form.notes" rows="2" class="w-full" placeholder="Optional notes" />
                </div>
                <div class="goal-dialog-footer">
                    <Button label="Cancel" outlined @click="showDialog = false" type="button" />
                    <Button
                        :label="editId !== null ? 'Update' : 'Add Goal'"
                        type="submit"
                        :loading="saving"
                        :disabled="!form.name || !form.targetAmount || !form.targetDate"
                    />
                </div>
            </form>
        </Dialog>

        <ConfirmDialog />
    </div>
</template>

<style scoped>
.goals-view {
    padding: 1.5rem;
    max-width: 1200px;
}

.goals-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    margin-bottom: 1.75rem;
    gap: 1rem;
}

.goals-title {
    font-size: 1.4rem;
    font-weight: 700;
    margin: 0 0 0.2rem;
}

.goals-subtitle {
    font-size: 0.875rem;
    color: var(--p-text-muted-color);
    margin: 0;
}

.goals-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.75rem;
    padding: 4rem 1rem;
    color: var(--p-text-muted-color);
    text-align: center;
}

.goals-empty-icon {
    font-size: 2.5rem;
    opacity: 0.35;
}

.goals-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 1.25rem;
}

/* ── goal card ───────────────────────────────────────────────── */

.goal-card {
    background: var(--p-content-background);
    border: 1px solid var(--p-content-border-color);
    border-radius: 10px;
    padding: 1.1rem 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    transition: box-shadow 0.15s;
}

.goal-card:hover {
    box-shadow: 0 2px 12px rgba(0, 0, 0, 0.09);
}

.goal-card--achieved {
    border-color: var(--p-green-500);
}

.goal-card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
}

.goal-card-title {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    min-width: 0;
}

.goal-cat-icon {
    font-size: 1.05rem;
    color: var(--p-primary-color);
    flex-shrink: 0;
}

.goal-name {
    font-weight: 600;
    font-size: 0.95rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.goal-card-actions {
    display: flex;
    gap: 0;
    flex-shrink: 0;
}

/* ── progress ────────────────────────────────────────────────── */

.goal-progress-wrap {
    display: flex;
    align-items: center;
    gap: 0.6rem;
}

.goal-bar {
    flex: 1;
    height: 8px;
    border-radius: 4px;
}

.goal-pct {
    font-size: 0.8rem;
    font-weight: 600;
    min-width: 3rem;
    text-align: right;
    color: var(--p-text-muted-color);
}

.goal-pct--achieved {
    color: var(--p-green-500);
}

/* ── amounts ─────────────────────────────────────────────────── */

.goal-amounts {
    display: flex;
    align-items: center;
    gap: 0.6rem;
}

.goal-amount-item {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
}

.goal-amount-label {
    font-size: 0.72rem;
    color: var(--p-text-muted-color);
    text-transform: uppercase;
    letter-spacing: 0.04em;
}

.goal-amount-value {
    font-size: 0.9rem;
    font-weight: 600;
}

.goal-amount-target {
    color: var(--p-primary-color);
}

.goal-arrow {
    font-size: 0.8rem;
    color: var(--p-text-muted-color);
    margin: 0.5rem 0 0;
}

/* ── footer ──────────────────────────────────────────────────── */

.goal-footer {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
}

.goal-date {
    font-size: 0.8rem;
    color: var(--p-text-muted-color);
}

.goal-cat-badge {
    margin-left: auto;
    font-size: 0.75rem;
    color: var(--p-text-muted-color);
    font-style: italic;
}

/* ── dialog form ─────────────────────────────────────────────── */

.goal-form {
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    padding-top: 0.25rem;
}

.goal-form .field {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
}

.goal-form .field label {
    font-size: 0.83rem;
    font-weight: 600;
}

.field-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.9rem;
}

.goal-cat-option {
    display: flex;
    align-items: center;
    gap: 0.5rem;
}

.goal-dialog-footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 0.25rem;
}
</style>
