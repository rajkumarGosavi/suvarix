import { createPinia, setActivePinia } from "pinia";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { useTransactionsStore } from "@/stores/transactions";

describe("useTransactionsStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  afterEach(() => {
    clearMocks();
  });

  it("fetch stores transactions, total count, and the active filter", async () => {
    mockIPC((cmd) => {
      if (cmd === "list_transactions") return [{ id: 1 }, { id: 2 }];
      if (cmd === "count_transactions") return 42;
      throw new Error(`unexpected command: ${cmd}`);
    });

    const store = useTransactionsStore();
    await store.fetch({ type: "expense", limit: 20 });

    expect(store.transactions).toEqual([{ id: 1 }, { id: 2 }]);
    expect(store.totalCount).toBe(42);
    expect(store.filter).toEqual({ type: "expense", limit: 20 });
    expect(store.isLoading).toBe(false);
  });

  it("fetch resets isLoading when the backend rejects", async () => {
    mockIPC(() => {
      throw new Error("backend exploded");
    });

    const store = useTransactionsStore();
    await expect(store.fetch()).rejects.toThrow("backend exploded");
    expect(store.isLoading).toBe(false);
  });

  it("add sends the payload then re-fetches with the previously active filter", async () => {
    const calls: Array<{ cmd: string; args: any }> = [];
    mockIPC((cmd, args) => {
      calls.push({ cmd, args });
      if (cmd === "list_transactions") return [{ id: 7 }];
      if (cmd === "count_transactions") return 1;
      if (cmd === "add_transaction") return 7;
      throw new Error(`unexpected command: ${cmd}`);
    });

    const store = useTransactionsStore();
    await store.fetch({ category: "Food" });
    await store.add({ amount: 100 });

    const addCall = calls.find((c) => c.cmd === "add_transaction");
    expect(addCall?.args).toEqual({ payload: { amount: 100 } });

    const listCalls = calls.filter((c) => c.cmd === "list_transactions");
    expect(listCalls).toHaveLength(2);
    expect(listCalls[1].args).toEqual({ filter: { category: "Food" } });
    expect(store.transactions).toEqual([{ id: 7 }]);
  });

  it("remove deletes by id then re-fetches", async () => {
    const calls: Array<{ cmd: string; args: any }> = [];
    mockIPC((cmd, args) => {
      calls.push({ cmd, args });
      if (cmd === "list_transactions") return [];
      if (cmd === "count_transactions") return 0;
      if (cmd === "delete_transaction") return null;
      throw new Error(`unexpected command: ${cmd}`);
    });

    const store = useTransactionsStore();
    await store.remove(5);

    expect(calls.find((c) => c.cmd === "delete_transaction")?.args).toEqual({ id: 5 });
    expect(calls.some((c) => c.cmd === "list_transactions")).toBe(true);
    expect(store.transactions).toEqual([]);
    expect(store.totalCount).toBe(0);
  });

  it("update edits by id then re-fetches", async () => {
    const calls: Array<{ cmd: string; args: any }> = [];
    mockIPC((cmd, args) => {
      calls.push({ cmd, args });
      if (cmd === "list_transactions") return [{ id: 5, amount: 999 }];
      if (cmd === "count_transactions") return 1;
      if (cmd === "update_transaction") return null;
      throw new Error(`unexpected command: ${cmd}`);
    });

    const store = useTransactionsStore();
    await store.update(5, { amount: 999 });

    expect(calls.find((c) => c.cmd === "update_transaction")?.args).toEqual({
      id: 5,
      payload: { amount: 999 },
    });
    expect(store.transactions).toEqual([{ id: 5, amount: 999 }]);
  });
});
