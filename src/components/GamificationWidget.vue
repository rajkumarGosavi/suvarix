<script setup lang="ts">
import { computed, onMounted } from "vue";
import { useGamificationStore } from "@/stores/gamification";

const store = useGamificationStore();
const stats = computed(() => store.stats);

onMounted(async () => {
    if (!store.stats) await store.fetch();
});

const levelSeverity = computed(() => {
    const map: Record<string, string> = {
        "Rookie": "secondary",
        "Saver": "info",
        "Investor": "success",
        "Pro Investor": "warn",
        "Market Expert": "contrast",
        "Finance Legend": "contrast",
    };
    return (map[stats.value?.level ?? "Rookie"] ?? "secondary") as any;
});
</script>

<template>
    <div class="gamification-widget card">
        <div class="gw-header">
            <span class="gw-title">Investor Journey</span>
            <Tag v-if="stats" :severity="levelSeverity" :value="stats.level" />
        </div>

        <template v-if="stats && stats.totalXp > 0">
            <div class="gw-xp-row">
                <ProgressBar
                    :value="stats.levelProgressPct"
                    class="gw-progress"
                    :show-value="false"
                />
                <span class="gw-xp-label">
                    {{ stats.totalXp }} XP
                    <span v-if="stats.nextLevelXpNeeded > 0" class="gw-xp-next">
                        &nbsp;· {{ stats.nextLevelXpNeeded }} to next level
                    </span>
                </span>
            </div>

            <div class="gw-streaks" v-if="store.txStreak && store.txStreak.currentCount > 0">
                <div class="gw-streak-item">
                    <span class="gw-streak-icon">📊</span>
                    <span class="gw-streak-count">{{ store.txStreak.currentCount }}</span>
                    <span class="gw-streak-label">week tx streak</span>
                </div>
            </div>

            <div class="gw-badges" v-if="store.recentBadges.length > 0">
                <span class="gw-badges-label">Recent badges</span>
                <div class="gw-badge-chips">
                    <Tag
                        v-for="badge in store.recentBadges"
                        :key="badge.id"
                        severity="secondary"
                        :value="`${badge.icon} ${badge.name}`"
                        class="gw-badge-chip"
                        v-tooltip.top="badge.description"
                    />
                </div>
            </div>
        </template>

        <div v-else class="gw-empty">
            <span>Start tracking to earn XP and unlock badges!</span>
        </div>
    </div>
</template>

<style scoped>
.gamification-widget {
    margin-bottom: 1.5rem;
}

.gw-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.75rem;
}

.gw-title {
    font-size: 1rem;
    font-weight: 600;
}

.gw-xp-row {
    margin-bottom: 0.75rem;
}

.gw-progress {
    width: 100%;
    height: 8px;
    margin-bottom: 0.35rem;
}

.gw-xp-label {
    font-size: 0.8rem;
    color: var(--p-text-muted-color);
}

.gw-xp-next {
    font-size: 0.75rem;
}

.gw-streaks {
    display: flex;
    gap: 1.5rem;
    margin-bottom: 0.75rem;
    flex-wrap: wrap;
}

.gw-streak-item {
    display: flex;
    align-items: center;
    gap: 0.3rem;
}

.gw-streak-icon {
    font-size: 1rem;
}

.gw-streak-count {
    font-size: 1.1rem;
    font-weight: 700;
}

.gw-streak-label {
    font-size: 0.8rem;
    color: var(--p-text-muted-color);
}

.gw-badges {
    margin-top: 0.25rem;
}

.gw-badges-label {
    font-size: 0.75rem;
    color: var(--p-text-muted-color);
    display: block;
    margin-bottom: 0.4rem;
}

.gw-badge-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
}

.gw-badge-chip {
    font-size: 0.78rem;
}

.gw-empty {
    font-size: 0.85rem;
    color: var(--p-text-muted-color);
    padding: 0.5rem 0;
}

@media (max-width: 639px) {
    .gw-streaks {
        gap: 1rem;
    }
}
</style>
