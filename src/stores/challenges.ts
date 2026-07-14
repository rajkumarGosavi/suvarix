import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

export interface ChallengeTemplate {
    kind: string;
    title: string;
    icon: string;
    description: string;
    defaultTarget: number;
    durationDays: number;
    xpReward: number;
    targetEditable: boolean;
    unit: string; // "₹" | "days" | ""
}

export interface ChallengeView {
    id: number;
    kind: string;
    title: string;
    icon: string;
    target: number;
    startDate: string;
    endDate: string;
    xpReward: number;
    status: "active" | "completed" | "failed";
    progressPct: number;
    progressValue: number;
    unit: string;
    daysLeft: number;
}

export const useChallengesStore = defineStore("challenges", {
    state: () => ({
        challenges: [] as ChallengeView[],
        templates: [] as ChallengeTemplate[],
        isLoading: false,
    }),
    getters: {
        active: (state) => state.challenges.filter((c) => c.status === "active"),
        finished: (state) => state.challenges.filter((c) => c.status !== "active"),
    },
    actions: {
        async fetch() {
            this.isLoading = true;
            try {
                this.challenges = await invoke<ChallengeView[]>("get_challenges");
            } catch {
                // non-fatal
            } finally {
                this.isLoading = false;
            }
        },
        async fetchTemplates() {
            try {
                this.templates = await invoke<ChallengeTemplate[]>("list_challenge_templates");
            } catch {
                this.templates = [];
            }
        },
        async join(kind: string, target: number | null) {
            await invoke("join_challenge", { kind, target });
            await Promise.all([this.fetch(), this.fetchTemplates()]);
        },
        async abandon(id: number) {
            this.challenges = this.challenges.filter((c) => c.id !== id);
            try {
                await invoke("abandon_challenge", { id });
            } finally {
                await this.fetchTemplates();
            }
        },
        // Transition active challenges; returns those that just completed (for celebration).
        async evaluate(): Promise<ChallengeView[]> {
            let completed: ChallengeView[] = [];
            try {
                completed = await invoke<ChallengeView[]>("evaluate_challenges");
            } catch {
                completed = [];
            }
            await this.fetch();
            return completed;
        },
    },
});
