<script setup lang="ts">
import { computed, watch } from "vue";
import { useRouter } from "vue-router";
import { useInsightsStore, type Nudge } from "@/stores/insights";
import { useAnalytics } from "@/composables/useAnalytics";

const store = useInsightsStore();
const router = useRouter();
const { track } = useAnalytics();

// Show the highest-priority handful; the rest stay one interaction away.
const MAX_VISIBLE = 4;
const visible = computed(() => store.nudges.slice(0, MAX_VISIBLE));

// Log which nudges the user actually saw — once per mount, when the feed first
// populates (data loads async after mount).
let loggedShown = false;
watch(
    visible,
    (v) => {
        if (loggedShown || v.length === 0) return;
        loggedShown = true;
        track("insight_feed_shown", { count: v.length, ids: v.map((n) => n.id) });
    },
    { immediate: true },
);

// Severity → accent colour (PrimeVue palette vars, dark-mode safe with hex fallback).
function accent(severity: Nudge["severity"]): string {
    switch (severity) {
        case "critical":
            return "var(--p-red-500, #ef4444)";
        case "warning":
            return "var(--p-amber-500, #f59e0b)";
        case "positive":
            return "var(--p-green-500, #22c55e)";
        default:
            return "var(--p-blue-500, #3b82f6)";
    }
}

function act(n: Nudge) {
    track("insight_clicked", { id: n.id, category: n.category, severity: n.severity });
    router.push(n.actionRoute);
}

function dismiss(n: Nudge) {
    track("insight_dismissed", { id: n.id, category: n.category, severity: n.severity });
    store.dismiss(n.id);
}
</script>

<template>
    <div class="insights card" v-if="visible.length > 0">
        <div class="in-header">
            <span class="in-title">Smart Insights</span>
            <span class="in-count" v-if="store.urgentCount > 0">{{ store.urgentCount }} to act on</span>
        </div>

        <div class="in-list">
            <div
                v-for="n in visible"
                :key="n.id"
                class="in-item"
                :style="{ borderLeftColor: accent(n.severity) }"
            >
                <span class="in-icon">{{ n.icon }}</span>
                <div class="in-body">
                    <span class="in-item-title">{{ n.title }}</span>
                    <span class="in-item-text">{{ n.body }}</span>
                    <div class="in-actions">
                        <Button
                            :label="n.actionLabel"
                            size="small"
                            text
                            class="in-act"
                            :style="{ color: accent(n.severity) }"
                            @click="act(n)"
                        />
                        <Button
                            label="Dismiss"
                            size="small"
                            text
                            severity="secondary"
                            class="in-dismiss"
                            @click="dismiss(n)"
                        />
                    </div>
                </div>
            </div>
        </div>
    </div>
</template>

<style scoped>
.insights {
    margin-bottom: 1.5rem;
}

.in-header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin-bottom: 1rem;
}

.in-title {
    font-size: 1rem;
    font-weight: 600;
}

.in-count {
    font-size: 0.75rem;
    font-weight: 600;
    color: var(--p-text-muted-color);
}

.in-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
}

.in-item {
    display: flex;
    gap: 0.75rem;
    padding: 0.75rem 0.9rem;
    border-radius: 8px;
    border-left: 3px solid transparent;
    background: var(--p-content-hover-background, rgba(0, 0, 0, 0.03));
}

.in-icon {
    font-size: 1.2rem;
    line-height: 1.4;
    flex-shrink: 0;
}

.in-body {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    min-width: 0;
    flex: 1;
}

.in-item-title {
    font-size: 0.88rem;
    font-weight: 600;
}

.in-item-text {
    font-size: 0.8rem;
    color: var(--p-text-muted-color);
}

.in-actions {
    display: flex;
    gap: 0.25rem;
    margin-top: 0.35rem;
}

.in-act,
.in-dismiss {
    padding: 0.2rem 0.5rem;
}
</style>
