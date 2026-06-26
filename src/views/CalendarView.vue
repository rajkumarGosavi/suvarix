<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useCurrencyFormat } from "@/composables/useCurrencyFormat";

interface CalendarEvent {
    date: string;
    eventType: string;
    sourceId: number;
    name: string;
    amount: number;
    category: string;
    isOverdue: boolean;
}

const { formatINR, formatCompact } = useCurrencyFormat();

const _today = new Date();
const todayStr = [
    _today.getFullYear(),
    String(_today.getMonth() + 1).padStart(2, "0"),
    String(_today.getDate()).padStart(2, "0"),
].join("-");

const currentYear  = ref(_today.getFullYear());
const currentMonth = ref(_today.getMonth() + 1); // 1-indexed
const events       = ref<CalendarEvent[]>([]);
const loading      = ref(false);
const selectedDay  = ref<string | null>(null);

const DOW_HEADERS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

const monthLabel = computed(() =>
    new Date(currentYear.value, currentMonth.value - 1, 1)
        .toLocaleDateString("en-IN", { month: "long", year: "numeric" })
);

async function loadEvents() {
    loading.value = true;
    try {
        events.value = await invoke<CalendarEvent[]>("get_calendar_events", {
            year: currentYear.value,
            month: currentMonth.value,
        });
    } catch {
        events.value = [];
    } finally {
        loading.value = false;
    }
}

function prevMonth() {
    if (currentMonth.value === 1) { currentMonth.value = 12; currentYear.value--; }
    else currentMonth.value--;
    selectedDay.value = null;
}

function nextMonth() {
    if (currentMonth.value === 12) { currentMonth.value = 1; currentYear.value++; }
    else currentMonth.value++;
    selectedDay.value = null;
}

function goToToday() {
    currentYear.value  = _today.getFullYear();
    currentMonth.value = _today.getMonth() + 1;
    selectedDay.value  = null;
}

// Calendar grid: null = blank leading cell, otherwise { day, dateStr }
const calendarDays = computed(() => {
    const firstDow = new Date(currentYear.value, currentMonth.value - 1, 1).getDay();
    const daysInMonth = new Date(currentYear.value, currentMonth.value, 0).getDate();
    const cells: ({ day: number; dateStr: string } | null)[] = [];
    for (let i = 0; i < firstDow; i++) cells.push(null);
    for (let d = 1; d <= daysInMonth; d++) {
        const dateStr = `${currentYear.value}-${String(currentMonth.value).padStart(2, "0")}-${String(d).padStart(2, "0")}`;
        cells.push({ day: d, dateStr });
    }
    return cells;
});

const eventsByDay = computed(() => {
    const map: Record<string, CalendarEvent[]> = {};
    for (const ev of events.value) {
        if (!map[ev.date]) map[ev.date] = [];
        map[ev.date].push(ev);
    }
    return map;
});

const selectedDayEvents = computed(() =>
    selectedDay.value ? (eventsByDay.value[selectedDay.value] ?? []) : []
);

const monthEventCount   = computed(() => events.value.length);
const monthTotalAmount  = computed(() => events.value.reduce((s, e) => s + e.amount, 0));

const overdueCount = computed(() =>
    events.value.filter(e => e.isOverdue).length
);

// Format selected day as "Thursday, 15 July 2026"
const selectedDayLabel = computed(() => {
    if (!selectedDay.value) return "";
    const [y, m, d] = selectedDay.value.split("-").map(Number);
    return new Date(y, m - 1, d).toLocaleDateString("en-IN", {
        weekday: "long", day: "numeric", month: "long", year: "numeric",
    });
});

// Event type styling
const TYPE_CONFIG: Record<string, { severity: string; icon: string; label: string }> = {
    loan:        { severity: "danger",    icon: "pi pi-home",        label: "Loan EMI" },
    credit_card: { severity: "warn",      icon: "pi pi-credit-card", label: "Card Payment" },
    bill:        { severity: "info",      icon: "pi pi-receipt",     label: "Bill" },
    goal:        { severity: "success",   icon: "pi pi-flag",        label: "Goal Target" },
    recurring:   { severity: "secondary", icon: "pi pi-refresh",     label: "Recurring" },
};

function typeConfig(t: string) {
    return TYPE_CONFIG[t] ?? { severity: "secondary", icon: "pi pi-circle", label: t };
}

watch([currentMonth, currentYear], loadEvents);
onMounted(loadEvents);
</script>

<template>
    <div class="calendar-view">
        <!-- Page header -->
        <div class="page-header">
            <h1 class="page-title">Calendar</h1>
            <div class="header-chips" v-if="!loading">
                <Tag :value="`${monthEventCount} event${monthEventCount !== 1 ? 's' : ''}`" severity="secondary" />
                <Tag :value="`${formatCompact(monthTotalAmount)} due`" severity="info" v-if="monthTotalAmount > 0" />
                <Tag :value="`${overdueCount} overdue`" severity="danger" v-if="overdueCount > 0" />
            </div>
        </div>

        <!-- Month navigation -->
        <div class="month-nav">
            <Button icon="pi pi-chevron-left" text @click="prevMonth" aria-label="Previous month" />
            <span class="month-label">{{ monthLabel }}</span>
            <Button icon="pi pi-chevron-right" text @click="nextMonth" aria-label="Next month" />
            <Button label="Today" outlined size="small" @click="goToToday" class="today-btn" />
        </div>

        <div v-if="loading" class="loading-state">
            <ProgressSpinner />
        </div>

        <div v-else class="calendar-layout">
            <!-- Grid -->
            <div class="cal-grid-wrap">
                <div class="cal-grid">
                    <!-- Day-of-week headers -->
                    <div v-for="h in DOW_HEADERS" :key="h" class="dow-header">{{ h }}</div>

                    <!-- Day cells -->
                    <template v-for="(cell, idx) in calendarDays" :key="idx">
                        <!-- Blank leading cell -->
                        <div v-if="cell === null" class="day-cell day-cell--blank" />

                        <!-- Actual day cell -->
                        <div
                            v-else
                            class="day-cell"
                            :class="{
                                'is-today':    cell.dateStr === todayStr,
                                'is-selected': cell.dateStr === selectedDay,
                                'has-events':  !!eventsByDay[cell.dateStr]?.length,
                            }"
                            @click="selectedDay = cell.dateStr"
                        >
                            <span class="day-num">{{ cell.day }}</span>

                            <div class="dot-row" v-if="eventsByDay[cell.dateStr]?.length">
                                <span
                                    v-for="(ev, i) in eventsByDay[cell.dateStr].slice(0, 3)"
                                    :key="i"
                                    class="ev-dot"
                                    :class="`ev-dot--${ev.eventType}`"
                                    :title="ev.name"
                                />
                                <span
                                    v-if="eventsByDay[cell.dateStr].length > 3"
                                    class="more-badge"
                                >+{{ eventsByDay[cell.dateStr].length - 3 }}</span>
                            </div>
                        </div>
                    </template>
                </div>

                <!-- Legend -->
                <div class="legend">
                    <div v-for="(cfg, type) in TYPE_CONFIG" :key="type" class="legend-item">
                        <span class="legend-dot" :class="`ev-dot--${type}`" />
                        <span class="legend-label">{{ cfg.label }}</span>
                    </div>
                </div>
            </div>

            <!-- Detail panel -->
            <div class="detail-panel" :class="{ 'detail-panel--open': selectedDay }">
                <template v-if="selectedDay">
                    <div class="detail-header">
                        <span class="detail-date">{{ selectedDayLabel }}</span>
                        <Button icon="pi pi-times" text size="small" @click="selectedDay = null" aria-label="Close" />
                    </div>
                    <Divider />

                    <div v-if="selectedDayEvents.length === 0" class="detail-empty">
                        <i class="pi pi-check-circle" />
                        <span>No events on this day</span>
                    </div>

                    <div v-else class="event-list">
                        <div
                            v-for="ev in selectedDayEvents"
                            :key="`${ev.eventType}-${ev.sourceId}-${ev.date}`"
                            class="event-item"
                        >
                            <div class="event-item-left">
                                <i :class="['event-icon', typeConfig(ev.eventType).icon]" />
                                <div class="event-info">
                                    <span class="event-name">{{ ev.name }}</span>
                                    <span class="event-category">{{ ev.category }}</span>
                                </div>
                            </div>
                            <div class="event-item-right">
                                <span class="event-amount">{{ formatINR(ev.amount) }}</span>
                                <Tag v-if="ev.isOverdue" value="Overdue" severity="danger" class="overdue-tag" />
                                <Tag :value="typeConfig(ev.eventType).label"
                                     :severity="typeConfig(ev.eventType).severity"
                                     class="type-tag" />
                            </div>
                        </div>
                    </div>
                </template>

                <div v-else class="detail-placeholder">
                    <i class="pi pi-calendar" />
                    <span>Click a day to see events</span>
                </div>
            </div>
        </div>
    </div>
</template>

<style scoped>
.calendar-view { max-width: 1200px; }

.page-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1.25rem;
    flex-wrap: wrap;
    gap: 0.75rem;
}
.page-title { font-size: 1.5rem; font-weight: 700; margin: 0; }
.header-chips { display: flex; gap: 0.5rem; flex-wrap: wrap; }

/* Month nav */
.month-nav {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 1.25rem;
}
.month-label {
    font-size: 1.15rem;
    font-weight: 700;
    min-width: 180px;
    text-align: center;
}
.today-btn { margin-left: 0.5rem; }

.loading-state { display: flex; justify-content: center; padding: 4rem; }

/* Two-column layout */
.calendar-layout {
    display: grid;
    grid-template-columns: 1fr 320px;
    gap: 1.25rem;
    align-items: start;
}

/* Calendar grid */
.cal-grid-wrap {
    background: var(--p-content-background);
    border: 1px solid var(--p-content-border-color);
    border-radius: 12px;
    overflow: hidden;
    padding-bottom: 0.75rem;
}

.cal-grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
}

.dow-header {
    padding: 0.6rem 0;
    text-align: center;
    font-size: 0.75rem;
    font-weight: 600;
    color: var(--p-text-muted-color);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    border-bottom: 1px solid var(--p-content-border-color);
}

.day-cell {
    min-height: 78px;
    padding: 0.4rem 0.45rem 0.35rem;
    border-right: 1px solid var(--p-content-border-color);
    border-bottom: 1px solid var(--p-content-border-color);
    cursor: pointer;
    transition: background 0.12s;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
}
.day-cell:nth-child(7n) { border-right: none; }
.day-cell--blank { cursor: default; background: var(--p-surface-ground); }
.day-cell:hover:not(.day-cell--blank) {
    background: color-mix(in srgb, var(--p-primary-color) 12%, transparent);
}

.day-cell.is-today .day-num {
    background: var(--p-primary-color);
    color: var(--p-primary-contrast-color);
    border-radius: 50%;
    width: 26px;
    height: 26px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: 700;
}
.day-cell.is-selected {
    background: color-mix(in srgb, var(--p-primary-color) 15%, transparent) !important;
    box-shadow: inset 0 0 0 2px var(--p-primary-color);
}

.day-num {
    font-size: 0.8rem;
    font-weight: 500;
    line-height: 1;
    width: 26px;
    height: 26px;
    display: flex;
    align-items: center;
    justify-content: center;
}

.dot-row {
    display: flex;
    flex-wrap: wrap;
    gap: 3px;
    align-items: center;
}

.ev-dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
}
.ev-dot--loan        { background: var(--p-red-500); }
.ev-dot--credit_card { background: var(--p-orange-500); }
.ev-dot--bill        { background: var(--p-blue-500); }
.ev-dot--goal        { background: var(--p-green-500); }
.ev-dot--recurring   { background: var(--p-surface-500); }

.more-badge {
    font-size: 0.65rem;
    color: var(--p-text-muted-color);
    line-height: 1;
}

/* Legend */
.legend {
    display: flex;
    gap: 1rem;
    flex-wrap: wrap;
    padding: 0.75rem 1rem 0;
    border-top: 1px solid var(--p-content-border-color);
    margin-top: 0.1rem;
}
.legend-item { display: flex; align-items: center; gap: 0.35rem; }
.legend-dot { width: 10px; height: 10px; border-radius: 50%; display: inline-block; }
.legend-label { font-size: 0.75rem; color: var(--p-text-muted-color); }

/* Detail panel */
.detail-panel {
    background: var(--p-content-background);
    border: 1px solid var(--p-content-border-color);
    border-radius: 12px;
    padding: 1rem;
    min-height: 300px;
    display: flex;
    flex-direction: column;
}

.detail-placeholder,
.detail-empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.6rem;
    color: var(--p-text-muted-color);
    font-size: 0.875rem;
}
.detail-placeholder i, .detail-empty i { font-size: 1.5rem; }

.detail-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 0.5rem;
}
.detail-date {
    font-size: 0.875rem;
    font-weight: 600;
    line-height: 1.4;
}

.event-list { display: flex; flex-direction: column; gap: 0.75rem; }

.event-item {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 0.5rem;
    padding: 0.6rem 0.75rem;
    border-radius: 8px;
    background: var(--p-surface-ground);
}
.event-item-left {
    display: flex;
    align-items: flex-start;
    gap: 0.6rem;
    flex: 1;
    min-width: 0;
}
.event-icon {
    font-size: 0.95rem;
    margin-top: 2px;
    color: var(--p-text-muted-color);
    flex-shrink: 0;
}
.event-info { display: flex; flex-direction: column; gap: 0.15rem; min-width: 0; }
.event-name { font-size: 0.85rem; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.event-category { font-size: 0.75rem; color: var(--p-text-muted-color); }

.event-item-right {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 0.25rem;
    flex-shrink: 0;
}
.event-amount { font-size: 0.85rem; font-weight: 600; }
.overdue-tag { font-size: 0.65rem; }
.type-tag { font-size: 0.65rem; }

/* Narrow screen: stack layout */
@media (max-width: 860px) {
    .calendar-layout { grid-template-columns: 1fr; }
}

@media (max-width: 639px) {
    .day-cell { min-height: 52px; padding: 0.25rem; }
    .day-num { font-size: 0.72rem; width: 22px; height: 22px; }
    .month-label { font-size: 1rem; min-width: 140px; }
    .legend { gap: 0.6rem; }
    .legend-label { font-size: 0.7rem; }
}
</style>
