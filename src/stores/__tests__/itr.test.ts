import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";

const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: any[]) => invokeMock(...args) }));

import { useItrStore, type ItrReturn } from "@/stores/itr";

function ret(ay: string): ItrReturn {
    return {
        id: null,
        assessmentYear: ay,
        formType: "ITR-2",
        regime: "old",
        panMasked: "XXXXX1234F",
        filingDate: "2024-07-20",
        ackNumber: "123456789012345",
        salaryIncome: 1250000,
        housePropertyIncome: 0,
        capitalGainsStcg: 0,
        capitalGainsLtcg: 0,
        otherSourcesIncome: 0,
        businessIncome: 0,
        grossTotalIncome: 1250000,
        chapterViaDeductions: 150000,
        totalIncome: 1100000,
        taxOnTotalIncome: 150000,
        surcharge: 0,
        cess: 6000,
        totalTaxLiability: 156000,
        tdsPaid: 156000,
        advanceTaxPaid: 0,
        selfAssessmentTaxPaid: 0,
        tcsPaid: 0,
        totalTaxPaid: 156000,
        refundDue: 0,
        taxPayable: 0,
        source: "pdf",
    };
}

describe("itr store", () => {
    beforeEach(() => {
        setActivePinia(createPinia());
        invokeMock.mockReset();
    });

    it("fetchAll loads returns and summary", async () => {
        invokeMock.mockImplementation((cmd: string) => {
            if (cmd === "list_itr_returns") return Promise.resolve([ret("2024-25")]);
            if (cmd === "get_itr_summary") return Promise.resolve({
                returnsCount: 1, lifetimeTaxPaid: 156000, lifetimeGrossIncome: 1250000,
                averageEffectiveRate: 12.48, latestAssessmentYear: "2024-25",
                latestTotalTaxLiability: 156000,
            });
            return Promise.reject(new Error(`unexpected ${cmd}`));
        });

        const store = useItrStore();
        await store.fetchAll();

        expect(store.returns).toHaveLength(1);
        expect(store.summary?.averageEffectiveRate).toBe(12.48);
        expect(store.isLoading).toBe(false);
    });

    it("save invokes save_itr_return then refreshes", async () => {
        invokeMock.mockImplementation((cmd: string) => {
            if (cmd === "save_itr_return") return Promise.resolve(1);
            if (cmd === "list_itr_returns") return Promise.resolve([ret("2024-25")]);
            if (cmd === "get_itr_summary") return Promise.resolve(null);
            return Promise.reject(new Error(`unexpected ${cmd}`));
        });

        const store = useItrStore();
        await store.save(ret("2024-25"));

        expect(invokeMock).toHaveBeenCalledWith("save_itr_return", { ret: ret("2024-25") });
        expect(invokeMock).toHaveBeenCalledWith("list_itr_returns");
        expect(store.returns).toHaveLength(1);
    });

    it("save records the error message and rethrows", async () => {
        invokeMock.mockRejectedValueOnce({ message: "Assessment year is required" });

        const store = useItrStore();
        await expect(store.save(ret(""))).rejects.toBeTruthy();
        expect(store.error).toBe("Assessment year is required");
        expect(store.isSaving).toBe(false);
    });

    it("remove invokes delete_itr_return then refreshes", async () => {
        invokeMock.mockImplementation((cmd: string) => {
            if (cmd === "delete_itr_return") return Promise.resolve();
            if (cmd === "list_itr_returns") return Promise.resolve([]);
            if (cmd === "get_itr_summary") return Promise.resolve(null);
            return Promise.reject(new Error(`unexpected ${cmd}`));
        });

        const store = useItrStore();
        await store.remove(7);

        expect(invokeMock).toHaveBeenCalledWith("delete_itr_return", { id: 7 });
        expect(store.returns).toHaveLength(0);
    });

    it("hasYear reports whether an assessment year is already stored", async () => {
        invokeMock.mockImplementation((cmd: string) =>
            cmd === "list_itr_returns" ? Promise.resolve([ret("2024-25")]) : Promise.resolve(null));

        const store = useItrStore();
        await store.fetchAll();

        expect(store.hasYear("2024-25")).toBe(true);
        expect(store.hasYear("2023-24")).toBe(false);
    });
});
