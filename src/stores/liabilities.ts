import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

export const useLiabilitiesStore = defineStore("liabilities", {
    state: () => ({
        loans: [] as any[],
        creditCards: [] as any[],
        isLoading: false,
    }),

    actions: {
        async fetchAll() {
            this.isLoading = true;
            try {
                const [loans, cards] = await Promise.all([
                    invoke<any[]>("list_loans"),
                    invoke<any[]>("list_credit_cards"),
                ]);
                this.loans = loans;
                this.creditCards = cards;
            } finally {
                this.isLoading = false;
            }
        },

        async addLoan(payload: any) {
            await invoke("add_loan", { payload });
            await this.fetchAll();
        },

        async updateLoan(id: number, payload: any) {
            await invoke("update_loan", { id, payload });
            await this.fetchAll();
        },

        async removeLoan(id: number) {
            await invoke("delete_loan", { id });
            await this.fetchAll();
        },

        async addCard(payload: any) {
            await invoke("add_credit_card", { payload });
            await this.fetchAll();
        },

        async updateCard(id: number, payload: any) {
            await invoke("update_credit_card", { id, payload });
            await this.fetchAll();
        },

        async removeCard(id: number) {
            await invoke("delete_credit_card", { id });
            await this.fetchAll();
        },
    },
});
