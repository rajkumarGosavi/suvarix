import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

export interface HealthPillar {
    key: string;
    label: string;
    score: number | null;
    weight: number;
    status: string;
    topFix: string;
    potentialGain: number;
}

export interface FinancialHealthScore {
    overall: number;
    grade: string;
    pillars: HealthPillar[];
}

export interface HealthSnapshotResult {
    previousScore: number | null;
    todayScore: number;
    alreadyRecordedToday: boolean;
}

export interface EmergencyFund {
    monthlyExpense: number;
    liquidAssets: number;
    targetMonths: number;
    targetAmount: number;
    monthsCovered: number;
    coveragePct: number;
    shortfall: number;
    status: "underfunded" | "on_track" | "funded";
}

export const useFinancialHealthStore = defineStore("financialHealth", {
    state: () => ({
        score: null as FinancialHealthScore | null,
        emergencyFund: null as EmergencyFund | null,
        isLoading: false,
    }),
    getters: {
        // Weakest computable pillar with an actionable fix — the "one thing to do next".
        topFixPillar: (state): HealthPillar | null => {
            const actionable = (state.score?.pillars ?? [])
                .filter((p) => p.score != null && p.topFix.length > 0)
                .sort((a, b) => (b.potentialGain ?? 0) - (a.potentialGain ?? 0));
            return actionable[0] ?? null;
        },
    },
    actions: {
        async fetch() {
            this.isLoading = true;
            try {
                this.score = await invoke<FinancialHealthScore>("get_financial_health");
            } catch {
                // non-fatal
            } finally {
                this.isLoading = false;
            }
        },
        async recordSnapshot(score: number): Promise<HealthSnapshotResult | null> {
            try {
                return await invoke<HealthSnapshotResult>("record_health_snapshot", { score });
            } catch {
                return null;
            }
        },
        async fetchEmergencyFund() {
            try {
                this.emergencyFund = await invoke<EmergencyFund>("get_emergency_fund");
            } catch {
                // non-fatal
            }
        },
        async setEmergencyTarget(months: number) {
            await invoke("set_emergency_fund_target", { months });
            await this.fetchEmergencyFund();
        },
    },
});
