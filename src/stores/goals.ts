import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

export interface Goal {
    id: number;
    name: string;
    category: string;
    targetAmount: number;
    targetDate: string;
    notes: string | null;
    achievedAt: string | null;
    createdAt: string;
    updatedAt: string;
}

export interface GoalPayload {
    name: string;
    category: string;
    targetAmount: number;
    targetDate: string;
    notes: string | null;
}

export const useGoalsStore = defineStore("goals", {
    state: () => ({
        goals: [] as Goal[],
        loading: false,
    }),
    actions: {
        async fetchGoals() {
            this.loading = true;
            try {
                this.goals = await invoke<Goal[]>("list_goals");
            } finally {
                this.loading = false;
            }
        },
        async addGoal(payload: GoalPayload) {
            await invoke("add_goal", { payload });
            await this.fetchGoals();
        },
        async updateGoal(id: number, payload: GoalPayload) {
            await invoke("update_goal", { id, payload });
            await this.fetchGoals();
        },
        async deleteGoal(id: number) {
            await invoke("delete_goal", { id });
            await this.fetchGoals();
        },
    },
});
