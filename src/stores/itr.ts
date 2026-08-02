import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

export interface ItrReturn {
    id: number | null;
    assessmentYear: string;
    formType: string;
    regime: string | null;
    panMasked: string | null;
    filingDate: string | null;
    ackNumber: string | null;

    salaryIncome: number;
    housePropertyIncome: number;
    capitalGainsStcg: number;
    capitalGainsLtcg: number;
    otherSourcesIncome: number;
    businessIncome: number;
    grossTotalIncome: number;

    chapterViaDeductions: number;
    totalIncome: number;

    taxOnTotalIncome: number;
    surcharge: number;
    cess: number;
    totalTaxLiability: number;

    tdsPaid: number;
    advanceTaxPaid: number;
    selfAssessmentTaxPaid: number;
    tcsPaid: number;
    totalTaxPaid: number;

    refundDue: number;
    taxPayable: number;

    source: "pdf" | "manual" | string;
}

export interface ItrSummary {
    returnsCount: number;
    lifetimeTaxPaid: number;
    lifetimeGrossIncome: number;
    averageEffectiveRate: number;
    latestAssessmentYear: string | null;
    latestTotalTaxLiability: number;
}

export const useItrStore = defineStore("itr", {
    state: () => ({
        returns: [] as ItrReturn[],
        summary: null as ItrSummary | null,
        isLoading: false,
        isSaving: false,
        error: "",
    }),

    getters: {
        hasYear: (state) => (assessmentYear: string) =>
            state.returns.some((r) => r.assessmentYear === assessmentYear),
    },

    actions: {
        async fetchAll() {
            this.isLoading = true;
            try {
                this.returns = await invoke<ItrReturn[]>("list_itr_returns");
                this.summary = await invoke<ItrSummary>("get_itr_summary");
            } finally {
                this.isLoading = false;
            }
        },

        async save(ret: ItrReturn) {
            this.isSaving = true;
            this.error = "";
            try {
                await invoke<number>("save_itr_return", { ret });
                await this.fetchAll();
            } catch (e: any) {
                this.error = e?.message ?? "Failed to save return.";
                throw e;
            } finally {
                this.isSaving = false;
            }
        },

        async remove(id: number) {
            this.error = "";
            try {
                await invoke("delete_itr_return", { id });
                await this.fetchAll();
            } catch (e: any) {
                this.error = e?.message ?? "Failed to delete return.";
                throw e;
            }
        },
    },
});
