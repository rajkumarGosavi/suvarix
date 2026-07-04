import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

export interface TransactionFilter {
    type?: string;
    assetClass?: string;
    accountId?: number;
    category?: string;
    dateFrom?: string;
    dateTo?: string;
    limit?: number;
    offset?: number;
}

export const useTransactionsStore = defineStore("transactions", {
    state: () => ({
        transactions: [] as any[],
        isLoading: false,
        filter: {} as TransactionFilter,
    }),

    actions: {
        async fetch(filter: TransactionFilter = {}) {
            this.isLoading = true;
            try {
                this.filter = filter;
                this.transactions = await invoke("list_transactions", { filter });
            } finally {
                this.isLoading = false;
            }
        },

        async add(payload: object) {
            await invoke("add_transaction", { payload });
            await this.fetch(this.filter);
        },

        async update(id: number, payload: object) {
            await invoke("update_transaction", { id, payload });
            await this.fetch(this.filter);
        },

        async remove(id: number) {
            await invoke("delete_transaction", { id });
            await this.fetch(this.filter);
        },
    },
});
