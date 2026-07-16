import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { usePortfolioStore } from "@/stores/portfolio";

export interface BrokerStatus {
    hasConfig: boolean;
    isConnected: boolean;
    tokenDate: string | null;
}

export interface SyncResult {
    synced: number;
    errors: string[];
}

interface BrokerCommands {
    status: string;
    saveConfig: string;
    connect: string;
    sync: string;
    disconnect: string;
}

/**
 * Shared store shape for broker integrations (Zerodha / Upstox / Angel One) —
 * identical state and actions, only the Tauri command names differ per broker.
 */
export function defineBrokerStore(id: string, cmd: BrokerCommands) {
    return defineStore(id, {
        state: () => ({
            status: null as BrokerStatus | null,
            connectLoading: false,
            syncLoading: false,
            syncResult: null as SyncResult | null,
            error: null as string | null,
        }),

        actions: {
            async fetchStatus() {
                try {
                    this.status = await invoke<BrokerStatus>(cmd.status);
                } catch (e: any) {
                    this.error = String(e?.message ?? e);
                }
            },

            async saveConfig(config: Record<string, string>) {
                this.error = null;
                try {
                    await invoke(cmd.saveConfig, config);
                    await this.fetchStatus();
                } catch (e: any) {
                    this.error = String(e?.message ?? e);
                    throw e;
                }
            },

            /** Zerodha/Upstox: no params (OAuth flow). Angel One: { password, totp }. */
            async connect(params: Record<string, string> = {}) {
                this.connectLoading = true;
                this.error = null;
                try {
                    await invoke(cmd.connect, params);
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
                    this.syncResult = await invoke<SyncResult>(cmd.sync);
                    // Refresh equity holdings in the portfolio view
                    await usePortfolioStore().fetchEquity();
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
                    await invoke(cmd.disconnect);
                    this.syncResult = null;
                    await this.fetchStatus();
                } catch (e: any) {
                    this.error = String(e?.message ?? e);
                    throw e;
                }
            },
        },
    });
}
