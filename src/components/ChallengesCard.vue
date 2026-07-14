<script setup lang="ts">
import { onMounted, ref, reactive } from "vue";
import { useToast } from "primevue/usetoast";
import { useChallengesStore, type ChallengeView, type ChallengeTemplate } from "@/stores/challenges";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";
import { useGamificationSafe } from "@/composables/useGamification";

const store = useChallengesStore();
const toast = useToast();
const { formatCompact } = useCurrencyFormat();
const { celebrate } = useGamificationSafe();

const pickerOpen = ref(false);
// Per-template target input for editable kinds (seeded with the default).
const targetInputs = reactive<Record<string, number>>({});

onMounted(async () => {
    await store.fetch();
    await store.fetchTemplates();
    const completed = await store.evaluate();
    for (const c of completed) {
        toast.add({
            severity: "success",
            summary: "Challenge complete! 🎉",
            detail: `${c.icon} ${c.title} — +${c.xpReward} XP`,
            life: 8000,
        });
        celebrate();
    }
});

function openPicker() {
    for (const t of store.templates) {
        if (t.targetEditable && targetInputs[t.kind] == null) {
            targetInputs[t.kind] = t.defaultTarget;
        }
    }
    pickerOpen.value = true;
}

async function join(t: ChallengeTemplate) {
    const target = t.targetEditable ? (targetInputs[t.kind] ?? t.defaultTarget) : null;
    try {
        await store.join(t.kind, target);
        pickerOpen.value = false;
        toast.add({ severity: "info", summary: "Challenge started!", detail: `${t.icon} ${t.title}`, life: 5000 });
    } catch {
        toast.add({ severity: "warn", summary: "Couldn't start challenge", life: 4000 });
    }
}

// Human progress line, currency-aware for the save challenge.
function progressLabel(c: ChallengeView): string {
    if (c.unit === "₹") return `${formatCompact(c.progressValue)} / ${formatCompact(c.target)}`;
    if (c.unit === "days") return `${c.progressValue} / ${c.target} no-spend days`;
    return c.status === "active" ? "On track — no budget breach" : "";
}

function barColor(c: ChallengeView): string {
    if (c.status === "completed") return "var(--p-green-500, #22c55e)";
    if (c.status === "failed") return "var(--p-red-500, #ef4444)";
    return "var(--p-primary-color, #3b82f6)";
}
</script>

<template>
    <div class="challenges card">
        <div class="ch-header">
            <span class="ch-title">Challenges</span>
            <Button
                v-if="store.templates.length > 0"
                icon="pi pi-plus"
                label="New"
                size="small"
                text
                @click="openPicker"
            />
        </div>

        <div v-if="store.active.length === 0 && store.finished.length === 0" class="ch-empty">
            <span>Take on a challenge to build better money habits. 🎯</span>
        </div>

        <div class="ch-list">
            <div v-for="c in store.challenges" :key="c.id" class="ch-item">
                <span class="ch-icon">{{ c.icon }}</span>
                <div class="ch-body">
                    <div class="ch-row">
                        <span class="ch-name">{{ c.title }}</span>
                        <span v-if="c.status === 'active'" class="ch-days">{{ c.daysLeft }}d left</span>
                        <span v-else-if="c.status === 'completed'" class="ch-badge ch-badge--done">Done +{{ c.xpReward }} XP</span>
                        <span v-else class="ch-badge ch-badge--fail">Missed</span>
                    </div>
                    <div class="ch-bar">
                        <div class="ch-bar-fill" :style="{ width: `${c.progressPct}%`, background: barColor(c) }" />
                    </div>
                    <div class="ch-row ch-sub">
                        <span class="ch-progress">{{ progressLabel(c) }}</span>
                        <Button
                            v-if="c.status === 'active'"
                            label="Abandon"
                            size="small"
                            text
                            severity="secondary"
                            class="ch-abandon"
                            @click="store.abandon(c.id)"
                        />
                    </div>
                </div>
            </div>
        </div>

        <Dialog v-model:visible="pickerOpen" modal header="Start a challenge" :style="{ width: '26rem', maxWidth: '92vw' }">
            <div class="ch-picker">
                <div v-for="t in store.templates" :key="t.kind" class="ch-tmpl">
                    <div class="ch-tmpl-head">
                        <span class="ch-icon">{{ t.icon }}</span>
                        <div class="ch-tmpl-text">
                            <span class="ch-name">{{ t.title }}</span>
                            <span class="ch-desc">{{ t.description }}</span>
                            <span class="ch-reward">+{{ t.xpReward }} XP · {{ t.durationDays }} days</span>
                        </div>
                    </div>
                    <div class="ch-tmpl-action">
                        <InputNumber
                            v-if="t.targetEditable"
                            v-model="targetInputs[t.kind]"
                            :min="1"
                            :prefix="t.unit === '₹' ? '₹' : undefined"
                            :suffix="t.unit === 'days' ? ' days' : undefined"
                            size="small"
                            class="ch-target"
                        />
                        <Button label="Start" size="small" @click="join(t)" />
                    </div>
                </div>
                <p v-if="store.templates.length === 0" class="ch-desc">All challenges are already active. 💪</p>
            </div>
        </Dialog>
    </div>
</template>

<style scoped>
.challenges {
    margin-bottom: 1.5rem;
}

.ch-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.75rem;
}

.ch-title {
    font-size: 1rem;
    font-weight: 600;
}

.ch-empty {
    font-size: 0.85rem;
    color: var(--p-text-muted-color);
    padding: 0.5rem 0;
}

.ch-list {
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
}

.ch-item {
    display: flex;
    gap: 0.75rem;
}

.ch-icon {
    font-size: 1.3rem;
    flex-shrink: 0;
    line-height: 1.3;
}

.ch-body {
    flex: 1;
    min-width: 0;
}

.ch-row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 0.5rem;
}

.ch-name {
    font-size: 0.88rem;
    font-weight: 600;
}

.ch-days {
    font-size: 0.72rem;
    color: var(--p-text-muted-color);
    flex-shrink: 0;
}

.ch-badge {
    font-size: 0.7rem;
    font-weight: 700;
    flex-shrink: 0;
}

.ch-badge--done {
    color: var(--p-green-500, #22c55e);
}

.ch-badge--fail {
    color: var(--p-red-500, #ef4444);
}

.ch-bar {
    width: 100%;
    height: 7px;
    border-radius: 4px;
    background: var(--p-content-border-color, #e5e7eb);
    overflow: hidden;
    margin: 0.35rem 0 0.25rem;
}

.ch-bar-fill {
    height: 100%;
    border-radius: 4px;
    transition: width 0.4s ease;
}

.ch-sub {
    margin-top: 0.1rem;
}

.ch-progress {
    font-size: 0.76rem;
    color: var(--p-text-muted-color);
}

.ch-abandon {
    padding: 0.1rem 0.4rem;
}

/* Picker dialog */
.ch-picker {
    display: flex;
    flex-direction: column;
    gap: 1rem;
}

.ch-tmpl {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding-bottom: 0.9rem;
    border-bottom: 1px solid var(--p-content-border-color);
}

.ch-tmpl:last-child {
    border-bottom: none;
    padding-bottom: 0;
}

.ch-tmpl-head {
    display: flex;
    gap: 0.6rem;
}

.ch-tmpl-text {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
}

.ch-desc {
    font-size: 0.78rem;
    color: var(--p-text-muted-color);
}

.ch-reward {
    font-size: 0.72rem;
    color: var(--p-primary-color);
    font-weight: 600;
}

.ch-tmpl-action {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    justify-content: flex-end;
}

.ch-target {
    max-width: 8rem;
}
</style>
