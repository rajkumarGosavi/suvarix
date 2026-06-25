import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

export interface CategorySummary {
    category: string;
    txType: "income" | "expense";
    total: number;
    count: number;
}

export interface BudgetStatus {
    category: string;
    monthlyLimit: number;
    spent: number;
    remaining: number;
    percentUsed: number;
}

export interface MonthlyTrend {
    month: string;
    income: number;
    expense: number;
}

export const useBudgetStore = defineStore("budget", {
    state: () => ({
        categorySummary: [] as CategorySummary[],
        budgetStatus: [] as BudgetStatus[],
        monthlyTrend: [] as MonthlyTrend[],
        isLoading: false,
        currentPeriod: "this_month",
    }),

    actions: {
        async fetchAll(period = "this_month") {
            this.currentPeriod = period;
            this.isLoading = true;
            try {
                const [summary, budgets, trend] = await Promise.all([
                    invoke<CategorySummary[]>("get_category_summary", { period }),
                    invoke<BudgetStatus[]>("get_budget_status"),
                    invoke<MonthlyTrend[]>("get_monthly_trend", { months: 12 }),
                ]);
                this.categorySummary = summary;
                this.budgetStatus = budgets;
                this.monthlyTrend = trend;
            } finally {
                this.isLoading = false;
            }
        },

        async setBudget(category: string, monthlyLimit: number) {
            await invoke("set_budget", { payload: { category, monthlyLimit } });
            await this.fetchAll(this.currentPeriod);
        },
    },
});
