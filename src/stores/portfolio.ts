import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

export interface NetWorthSummary {
    totalAssets: number;
    totalLiabilities: number;
    netWorth: number;
}

export interface AllocationItem {
    label: string;
    value: number;
    percent: number;
}

export const usePortfolioStore = defineStore("portfolio", {
    state: () => ({
        netWorth: null as NetWorthSummary | null,
        allocation: [] as AllocationItem[],
        isLoading: false,
        equity: [] as any[],
        mf: [] as any[],
        fd: [] as any[],
        ppfEpf: [] as any[],
        realEstate: [] as any[],
        gold: [] as any[],
        crypto: [] as any[],
        insurance: [] as any[],
    }),

    actions: {
        async fetchNetWorth() {
            this.netWorth = await invoke<NetWorthSummary>("get_net_worth");
        },

        async fetchAllocation() {
            this.allocation = await invoke<AllocationItem[]>("get_allocation_breakdown");
        },

        async fetchEquity() {
            this.equity = await invoke("list_equity");
        },
        async fetchMf() {
            this.mf = await invoke("list_mf");
        },
        async fetchFd() {
            this.fd = await invoke("list_fd");
        },
        async fetchPpfEpf() {
            this.ppfEpf = await invoke("list_ppf_epf");
        },
        async fetchRealEstate() {
            this.realEstate = await invoke("list_real_estate");
        },
        async fetchGold() {
            this.gold = await invoke("list_gold");
        },
        async fetchCrypto() {
            this.crypto = await invoke("list_crypto");
        },
        async fetchInsurance() {
            this.insurance = await invoke("list_insurance");
        },

        async fetchAll() {
            this.isLoading = true;
            try {
                await Promise.all([
                    this.fetchNetWorth(),
                    this.fetchAllocation(),
                    this.fetchEquity(),
                    this.fetchMf(),
                    this.fetchFd(),
                    this.fetchPpfEpf(),
                    this.fetchRealEstate(),
                    this.fetchGold(),
                    this.fetchCrypto(),
                    this.fetchInsurance(),
                ]);
            } finally {
                this.isLoading = false;
            }
        },
    },
});
