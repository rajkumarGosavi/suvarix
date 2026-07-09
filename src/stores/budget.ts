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
        currentCustomStart: null as string | null,
        currentCustomEnd: null as string | null,
        trendMonths: 12,
    }),

    actions: {
        async fetchAll(period = "this_month", customStart: string | null = null, customEnd: string | null = null) {
            this.currentPeriod = period;
            this.currentCustomStart = customStart;
            this.currentCustomEnd = customEnd;
            this.isLoading = true;
            try {
                const [summary, budgets, trend] = await Promise.all([
                    invoke<CategorySummary[]>("get_category_summary", { period, customStart, customEnd }),
                    invoke<BudgetStatus[]>("get_budget_status"),
                    invoke<MonthlyTrend[]>("get_monthly_trend", { months: this.trendMonths }),
                ]);
                this.categorySummary = summary;
                this.budgetStatus = budgets;
                this.monthlyTrend = trend;
            } finally {
                this.isLoading = false;
            }
        },

        async fetchTrend(months: number) {
            this.trendMonths = months;
            this.monthlyTrend = await invoke<MonthlyTrend[]>("get_monthly_trend", { months });
        },

        async setBudget(category: string, monthlyLimit: number) {
            await invoke("set_budget", { payload: { category, monthlyLimit } });
            await this.fetchAll(this.currentPeriod, this.currentCustomStart, this.currentCustomEnd);
        },

        async deleteBudget(category: string) {
            await invoke("delete_budget", { category });
            await this.fetchAll(this.currentPeriod, this.currentCustomStart, this.currentCustomEnd);
        },
    },
});
