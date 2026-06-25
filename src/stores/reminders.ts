import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

export interface Bill {
    id: number;
    name: string;
    category: string;
    amount: number;
    frequency: string;
    nextDueDate: string;
    notes: string | null;
    isActive: number;
    createdAt: string;
}

export interface BillPayload {
    name: string;
    category: string;
    amount: number;
    frequency: string;
    nextDueDate: string;
    notes: string | null;
}

export interface RecurringTx {
    id: number;
    name: string;
    type: string;
    amount: number;
    category: string;
    assetClass: string | null;
    description: string | null;
    notes: string | null;
    frequency: string;
    nextDueDate: string;
    lastRunDate: string | null;
    isActive: number;
    createdAt: string;
}

export interface RecurringTxPayload {
    name: string;
    type: string;
    amount: number;
    category: string;
    assetClass: string | null;
    description: string | null;
    notes: string | null;
    frequency: string;
    nextDueDate: string;
}

export interface UpcomingReminder {
    source: string;
    sourceId: number;
    name: string;
    amount: number;
    dueDate: string;
    category: string;
    daysUntilDue: number;
}

export const useRemindersStore = defineStore("reminders", {
    state: () => ({
        bills: [] as Bill[],
        upcomingReminders: [] as UpcomingReminder[],
        recurringList: [] as RecurringTx[],
        dueRecurring: [] as RecurringTx[],
        loading: false,
    }),
    actions: {
        async fetchBills() {
            this.bills = await invoke<Bill[]>("list_bills");
        },
        async addBill(payload: BillPayload) {
            await invoke("add_bill", { payload });
            await this.fetchBills();
        },
        async updateBill(id: number, payload: BillPayload) {
            await invoke("update_bill", { id, payload });
            await this.fetchBills();
        },
        async deleteBill(id: number) {
            await invoke("delete_bill", { id });
            await this.fetchBills();
        },
        async loadUpcoming(days = 30) {
            this.upcomingReminders = await invoke<UpcomingReminder[]>("get_upcoming_reminders", { days });
        },
        async markPaid(source: string, sourceId: number, amount: number, date: string, notes: string | null) {
            await invoke("mark_reminder_paid", { source, sourceId, amount, date, notes });
            await this.loadUpcoming(30);
        },
        async fetchRecurring() {
            this.recurringList = await invoke<RecurringTx[]>("list_recurring");
        },
        async addRecurring(payload: RecurringTxPayload) {
            await invoke("add_recurring", { payload });
            await this.fetchRecurring();
        },
        async updateRecurring(id: number, payload: RecurringTxPayload) {
            await invoke("update_recurring", { id, payload });
            await this.fetchRecurring();
        },
        async deleteRecurring(id: number) {
            await invoke("delete_recurring", { id });
            await this.fetchRecurring();
        },
        async toggleRecurring(id: number) {
            await invoke("toggle_recurring", { id });
            await this.fetchRecurring();
        },
        async loadDue() {
            this.dueRecurring = await invoke<RecurringTx[]>("get_due_recurring");
        },
        async applyDue(ids: number[]) {
            await invoke("apply_recurring", { ids });
            await this.fetchRecurring();
            await this.loadDue();
        },
    },
});
