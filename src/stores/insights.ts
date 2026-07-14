import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

export type NudgeSeverity = "critical" | "warning" | "positive" | "info";

export interface Nudge {
    id: string;
    severity: NudgeSeverity;
    category: string;
    icon: string;
    title: string;
    body: string;
    actionLabel: string;
    actionRoute: string;
    priority: number;
}

export const useInsightsStore = defineStore("insights", {
    state: () => ({
        nudges: [] as Nudge[],
        isLoading: false,
    }),
    getters: {
        // Anything actionable/urgent — used for a badge count on the nav if wanted.
        urgentCount: (state) =>
            state.nudges.filter((n) => n.severity === "critical" || n.severity === "warning").length,
    },
    actions: {
        async fetch() {
            this.isLoading = true;
            try {
                this.nudges = await invoke<Nudge[]>("get_insights");
            } catch {
                // non-fatal — feed just stays empty
            } finally {
                this.isLoading = false;
            }
        },
        // Optimistically drop the card, then persist the 7-day dismissal.
        async dismiss(id: string) {
            this.nudges = this.nudges.filter((n) => n.id !== id);
            try {
                await invoke("dismiss_insight", { id });
            } catch {
                // non-fatal — it'll reappear on next fetch if the write failed
            }
        },
    },
});
