import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

export interface NetWorthSnapshot {
    snapshotDate: string;
    totalAssets: number;
    totalLiabilities: number;
    netWorth: number;
}

export interface GainTxn {
    symbol: string;
    assetClass: string;
    buyDate: string;
    sellDate: string;
    quantity: number;
    buyPrice: number;
    sellPrice: number;
    gain: number;
    gainType: "STCG" | "LTCG";
    holdingDays: number;
}

export interface CapitalGainsSummary {
    stcg: number;
    ltcg: number;
    transactions: GainTxn[];
}

export const useReportsStore = defineStore("reports", {
    state: () => ({
        snapshots: [] as NetWorthSnapshot[],
        capitalGains: null as CapitalGainsSummary | null,
        isLoadingHistory: false,
        isLoadingGains: false,
        isTakingSnapshot: false,
        snapshotError: "",
    }),

    actions: {
        async fetchHistory(months = 12) {
            this.isLoadingHistory = true;
            try {
                this.snapshots = await invoke<NetWorthSnapshot[]>("get_net_worth_history", { months });
            } finally {
                this.isLoadingHistory = false;
            }
        },

        async takeSnapshot() {
            this.isTakingSnapshot = true;
            this.snapshotError = "";
            try {
                await invoke("take_net_worth_snapshot");
                await this.fetchHistory();
            } catch (e: any) {
                this.snapshotError = e?.message ?? "Failed to take snapshot.";
            } finally {
                this.isTakingSnapshot = false;
            }
        },

        async fetchCapitalGains(fy: string, method: string) {
            this.isLoadingGains = true;
            try {
                this.capitalGains = await invoke<CapitalGainsSummary>("get_capital_gains", { fy, method });
            } finally {
                this.isLoadingGains = false;
            }
        },
    },
});
