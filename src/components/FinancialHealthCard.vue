<script setup lang="ts">
import { computed, ref } from "vue";
import { useFinancialHealthStore, type HealthPillar } from "@/stores/financialHealth";

const store = useFinancialHealthStore();
const expanded = ref(false);

const score = computed(() => store.score);

// Grade → colour. Uses PrimeVue palette vars (theme-aware, dark-mode safe) with hex fallback.
function gradeColorVar(grade: string): string {
    switch (grade) {
        case "A+":
        case "A":
            return "var(--p-green-500, #22c55e)";
        case "B":
            return "var(--p-teal-500, #14b8a6)";
        case "C":
            return "var(--p-amber-500, #f59e0b)";
        default:
            return "var(--p-red-500, #ef4444)";
    }
}

const ringColor = computed(() => gradeColorVar(score.value?.grade ?? "D"));

// Conic-gradient ring fill for the overall score.
const ringStyle = computed(() => {
    const pct = score.value?.overall ?? 0;
    const col = ringColor.value;
    return {
        background: `conic-gradient(${col} ${pct * 3.6}deg, var(--p-content-border-color, #e5e7eb) 0deg)`,
    };
});

function pillarColor(p: HealthPillar): string {
    const s = p.score ?? 0;
    if (s >= 70) return "var(--p-green-500, #22c55e)";
    if (s >= 45) return "var(--p-amber-500, #f59e0b)";
    return "var(--p-red-500, #ef4444)";
}

const topFix = computed(() => store.topFixPillar);
</script>

<template>
    <div class="health-card card" v-if="score">
        <div class="hc-header">
            <span class="hc-title">Financial Health</span>
            <span class="hc-grade" :style="{ color: ringColor }">{{ score.grade }}</span>
        </div>

        <div class="hc-body">
            <div class="hc-ring" :style="ringStyle">
                <div class="hc-ring-inner">
                    <span class="hc-score">{{ Math.round(score.overall) }}</span>
                    <span class="hc-score-max">/ 100</span>
                </div>
            </div>

            <div class="hc-summary">
                <div v-if="topFix" class="hc-topfix">
                    <span class="hc-topfix-label">Do this next</span>
                    <span class="hc-topfix-text">{{ topFix.topFix }}</span>
                    <span class="hc-topfix-gain">up to +{{ topFix.potentialGain }} pts</span>
                </div>
                <div v-else class="hc-topfix hc-topfix--good">
                    <span class="hc-topfix-text">Great shape — keep it up! 🎉</span>
                </div>

                <Button
                    :label="expanded ? 'Hide breakdown' : 'View breakdown'"
                    text
                    size="small"
                    class="hc-toggle"
                    @click="expanded = !expanded"
                />
            </div>
        </div>

        <div class="hc-pillars" v-if="expanded">
            <div v-for="p in score.pillars" :key="p.key" class="hc-pillar">
                <div class="hc-pillar-top">
                    <span class="hc-pillar-label">{{ p.label }}</span>
                    <span class="hc-pillar-score" v-if="p.score != null">
                        {{ Math.round(p.score) }}
                    </span>
                    <span class="hc-pillar-score hc-pillar-score--na" v-else>—</span>
                </div>
                <div class="hc-bar" v-if="p.score != null">
                    <div
                        class="hc-bar-fill"
                        :style="{ width: `${p.score}%`, background: pillarColor(p) }"
                    />
                </div>
                <span class="hc-pillar-status">{{ p.status }}</span>
                <span class="hc-pillar-fix" v-if="p.topFix">→ {{ p.topFix }}</span>
            </div>
        </div>
    </div>
</template>

<style scoped>
.health-card {
    margin-bottom: 1.5rem;
}

.hc-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 1rem;
}

.hc-title {
    font-size: 1rem;
    font-weight: 600;
}

.hc-grade {
    font-size: 1.4rem;
    font-weight: 800;
    letter-spacing: -0.02em;
    flex-shrink: 0;
    white-space: nowrap;
    padding-left: 0.5rem;
}

.hc-body {
    display: flex;
    align-items: center;
    gap: 1.5rem;
}

.hc-ring {
    width: 108px;
    height: 108px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    flex-shrink: 0;
}

.hc-ring-inner {
    width: 82px;
    height: 82px;
    border-radius: 50%;
    background: var(--p-content-background, #fff);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    line-height: 1;
}

.hc-score {
    font-size: 1.9rem;
    font-weight: 800;
}

.hc-score-max {
    font-size: 0.7rem;
    color: var(--p-text-muted-color);
    margin-top: 0.15rem;
}

.hc-summary {
    flex: 1;
    min-width: 0;
}

.hc-topfix {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    margin-bottom: 0.5rem;
}

.hc-topfix-label {
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--p-text-muted-color);
}

.hc-topfix-text {
    font-size: 0.9rem;
    font-weight: 500;
}

.hc-topfix-gain {
    font-size: 0.75rem;
    color: var(--p-primary-color);
    font-weight: 600;
}

.hc-topfix--good .hc-topfix-text {
    color: var(--p-green-500, #22c55e);
}

.hc-toggle {
    padding-left: 0;
}

.hc-pillars {
    margin-top: 1rem;
    padding-top: 1rem;
    border-top: 1px solid var(--p-content-border-color);
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
}

.hc-pillar {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
}

.hc-pillar-top {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
}

.hc-pillar-label {
    font-size: 0.85rem;
    font-weight: 600;
}

.hc-pillar-score {
    font-size: 0.85rem;
    font-weight: 700;
}

.hc-pillar-score--na {
    color: var(--p-text-muted-color);
    font-weight: 500;
}

.hc-bar {
    width: 100%;
    height: 6px;
    border-radius: 3px;
    background: var(--p-content-border-color, #e5e7eb);
    overflow: hidden;
}

.hc-bar-fill {
    height: 100%;
    border-radius: 3px;
    transition: width 0.4s ease;
}

.hc-pillar-status {
    font-size: 0.78rem;
    color: var(--p-text-muted-color);
}

.hc-pillar-fix {
    font-size: 0.78rem;
    color: var(--p-primary-color);
}

@media (max-width: 639px) {
    .hc-body {
        flex-direction: column;
        align-items: stretch;
        gap: 1rem;
    }

    .hc-ring {
        align-self: center;
    }
}
</style>
