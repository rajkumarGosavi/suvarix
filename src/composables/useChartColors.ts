import { computed } from "vue";
import { useUiStore } from "@/stores/ui";

function cssVar(name: string, fallback: string): string {
    const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    return v || fallback;
}

export function useChartColors() {
    const ui = useUiStore();

    const textColor = computed(() => {
        void ui.theme;
        return cssVar("--p-text-color", "#374151");
    });

    const mutedColor = computed(() => {
        void ui.theme;
        return cssVar("--p-text-muted-color", "#9ca3af");
    });

    const gridColor = computed(() => {
        void ui.theme;
        return cssVar("--p-content-border-color", "#e5e7eb");
    });

    const PALETTE = [
        "#3b82f6", // Equity  — blue
        "#8b5cf6", // MF      — purple
        "#06b6d4", // FD      — cyan
        "#10b981", // PPF/EPF — emerald
        "#f59e0b", // Real Est — amber
        "#eab308", // Gold    — yellow
        "#f97316", // Crypto  — orange
        "#ef4444", // Ins.    — red
    ];

    return { textColor, mutedColor, gridColor, PALETTE };
}
