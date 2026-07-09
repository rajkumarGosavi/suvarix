import { createPinia, setActivePinia } from "pinia";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { useLiabilitiesStore } from "@/stores/liabilities";

describe("useLiabilitiesStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  afterEach(() => {
    clearMocks();
  });

  it("fetchAll loads loans and credit cards in parallel and toggles isLoading", async () => {
    mockIPC((cmd) => {
      if (cmd === "list_loans") return [{ id: 1, lenderName: "HDFC" }];
      if (cmd === "list_credit_cards") return [{ id: 2, bankName: "ICICI" }];
      throw new Error(`unexpected command: ${cmd}`);
    });

    const store = useLiabilitiesStore();
    const pending = store.fetchAll();
    expect(store.isLoading).toBe(true);
    await pending;

    expect(store.isLoading).toBe(false);
    expect(store.loans).toEqual([{ id: 1, lenderName: "HDFC" }]);
    expect(store.creditCards).toEqual([{ id: 2, bankName: "ICICI" }]);
  });

  it("fetchAll resets isLoading when the backend rejects", async () => {
    mockIPC(() => {
      throw new Error("backend exploded");
    });

    const store = useLiabilitiesStore();
    await expect(store.fetchAll()).rejects.toThrow("backend exploded");
    expect(store.isLoading).toBe(false);
  });

  it("addLoan sends the payload then re-fetches both lists", async () => {
    const calls: Array<{ cmd: string; args: any }> = [];
    mockIPC((cmd, args) => {
      calls.push({ cmd, args });
      if (cmd === "list_loans") return [{ id: 1 }];
      if (cmd === "list_credit_cards") return [];
      if (cmd === "add_loan") return 1;
      throw new Error(`unexpected command: ${cmd}`);
    });

    const store = useLiabilitiesStore();
    await store.addLoan({ lenderName: "HDFC", principal: 100000 });

    expect(calls.find((c) => c.cmd === "add_loan")?.args).toEqual({
      payload: { lenderName: "HDFC", principal: 100000 },
    });
    expect(store.loans).toEqual([{ id: 1 }]);
  });

  it("removeCard deletes by id then re-fetches", async () => {
    const calls: Array<{ cmd: string; args: any }> = [];
    mockIPC((cmd, args) => {
      calls.push({ cmd, args });
      if (cmd === "list_loans") return [];
      if (cmd === "list_credit_cards") return [];
      if (cmd === "delete_credit_card") return null;
      throw new Error(`unexpected command: ${cmd}`);
    });

    const store = useLiabilitiesStore();
    await store.removeCard(9);

    expect(calls.find((c) => c.cmd === "delete_credit_card")?.args).toEqual({ id: 9 });
    expect(store.creditCards).toEqual([]);
  });
});
