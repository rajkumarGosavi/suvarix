import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { usePortfolioStore } from "@/stores/portfolio";

interface ZerodhaStatus {
    hasConfig: boolean;
    isConnected: boolean;
    tokenDate: string | null;
}

interface SyncResult {
    synced: number;
    errors: string[];
}

export const useZerodhaStore = defineStore("zerodha", {
    state: () => ({
        status: null as ZerodhaStatus | null,
        connectLoading: false,
        syncLoading: false,
        syncResult: null as SyncResult | null,
        error: null as string | null,
    }),

    actions: {
        async fetchStatus() {
            try {
                this.status = await invoke<ZerodhaStatus>("get_zerodha_status");
            } catch (e: any) {
                this.error = String(e?.message ?? e);
            }
        },

        async saveConfig(apiKey: string, apiSecret: string) {
            this.error = null;
            try {
                await invoke("save_zerodha_config", { apiKey, apiSecret });
                await this.fetchStatus();
            } catch (e: any) {
                this.error = String(e?.message ?? e);
                throw e;
            }
        },

        async connect() {
            this.connectLoading = true;
            this.error = null;
            try {
                await invoke("start_zerodha_login");
                await this.fetchStatus();
            } catch (e: any) {
                this.error = String(e?.message ?? e);
                throw e;
            } finally {
                this.connectLoading = false;
            }
        },

        async syncHoldings() {
            this.syncLoading = true;
            this.syncResult = null;
            this.error = null;
            try {
                const result = await invoke<SyncResult>("sync_zerodha_holdings");
                this.syncResult = result;
                // Refresh equity holdings in the portfolio view
                const portfolio = usePortfolioStore();
                await portfolio.fetchEquity();
            } catch (e: any) {
                this.error = String(e?.message ?? e);
                throw e;
            } finally {
                this.syncLoading = false;
            }
        },

        async disconnect() {
            this.error = null;
            try {
                await invoke("disconnect_zerodha");
                this.syncResult = null;
                await this.fetchStatus();
            } catch (e: any) {
                this.error = String(e?.message ?? e);
                throw e;
            }
        },
    },
});
