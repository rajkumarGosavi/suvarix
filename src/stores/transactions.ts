import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

export interface TransactionFilter {
    type?: string;
    assetClass?: string;
    accountId?: number;
    category?: string;
    dateFrom?: string;
    dateTo?: string;
    search?: string;
    sortBy?: "date" | "amount";
    sortDir?: "asc" | "desc";
    limit?: number;
    offset?: number;
}

export const useTransactionsStore = defineStore("transactions", {
    state: () => ({
        transactions: [] as any[],
        totalCount: 0,
        isLoading: false,
        filter: {} as TransactionFilter,
    }),

    actions: {
        async fetch(filter: TransactionFilter = {}) {
            this.isLoading = true;
            try {
                this.filter = filter;
                const [transactions, totalCount] = await Promise.all([
                    invoke<any[]>("list_transactions", { filter }),
                    invoke<number>("count_transactions", { filter }),
                ]);
                this.transactions = transactions;
                this.totalCount = totalCount;
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
