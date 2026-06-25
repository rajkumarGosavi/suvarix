import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

export interface RefreshResult {
    updated: number;
    failed: number;
    errors: string[];
}

export interface MarketIndices {
    nifty50: number | null;
    sensex: number | null;
    usdInr: number | null;
    lastUpdated: string | null;
}

export const usePricesStore = defineStore("prices", {
    state: () => ({
        indices: null as MarketIndices | null,
        indicesLoading: false,
        equityResult: null as RefreshResult | null,
        equityLoading: false,
        mfResult: null as RefreshResult | null,
        mfLoading: false,
    }),

    actions: {
        async fetchIndices() {
            this.indicesLoading = true;
            try {
                this.indices = await invoke<MarketIndices>("get_market_indices");
            } finally {
                this.indicesLoading = false;
            }
        },

        async refreshEquity() {
            this.equityLoading = true;
            this.equityResult = null;
            try {
                this.equityResult = await invoke<RefreshResult>("refresh_equity_prices");
            } finally {
                this.equityLoading = false;
            }
        },

        async refreshMfNav() {
            this.mfLoading = true;
            this.mfResult = null;
            try {
                this.mfResult = await invoke<RefreshResult>("refresh_mf_navs");
            } finally {
                this.mfLoading = false;
            }
        },
    },
});
