import { createPinia, setActivePinia } from "pinia";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { usePortfolioStore } from "@/stores/portfolio";

describe("usePortfolioStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  afterEach(() => {
    clearMocks();
  });

  it("fetchNetWorth stores the net worth summary returned by the backend", async () => {
    mockIPC((cmd) => {
      if (cmd === "get_net_worth") {
        return { totalAssets: 100000, totalLiabilities: 20000, netWorth: 80000 };
      }
      throw new Error(`unexpected command: ${cmd}`);
    });

    const store = usePortfolioStore();
    await store.fetchNetWorth();

    expect(store.netWorth).toEqual({ totalAssets: 100000, totalLiabilities: 20000, netWorth: 80000 });
  });

  it("fetchAllocation stores the allocation breakdown returned by the backend", async () => {
    mockIPC((cmd) => {
      if (cmd === "get_allocation_breakdown") {
        return [{ label: "Equity", value: 50000, percent: 62.5 }];
      }
      throw new Error(`unexpected command: ${cmd}`);
    });

    const store = usePortfolioStore();
    await store.fetchAllocation();

    expect(store.allocation).toEqual([{ label: "Equity", value: 50000, percent: 62.5 }]);
  });

  it("fetchAll fetches every asset class in parallel and toggles isLoading", async () => {
    const listCommands = [
      "list_equity",
      "list_mf",
      "list_fd",
      "list_ppf_epf",
      "list_real_estate",
      "list_gold",
      "list_crypto",
      "list_insurance",
      "list_bonds",
    ];

    mockIPC((cmd) => {
      if (cmd === "get_net_worth") {
        return { totalAssets: 1, totalLiabilities: 0, netWorth: 1 };
      }
      if (cmd === "get_allocation_breakdown") {
        return [];
      }
      if (listCommands.includes(cmd)) {
        return [{ id: 1 }];
      }
      throw new Error(`unexpected command: ${cmd}`);
    });

    const store = usePortfolioStore();
    const pending = store.fetchAll();
    expect(store.isLoading).toBe(true);
    await pending;

    expect(store.isLoading).toBe(false);
    expect(store.netWorth).toEqual({ totalAssets: 1, totalLiabilities: 0, netWorth: 1 });
    expect(store.equity).toEqual([{ id: 1 }]);
    expect(store.bonds).toEqual([{ id: 1 }]);
  });

  it("fetchAll resets isLoading even when one of the invokes rejects", async () => {
    mockIPC((cmd) => {
      if (cmd === "get_net_worth") {
        throw new Error("backend exploded");
      }
      return [];
    });

    const store = usePortfolioStore();
    await expect(store.fetchAll()).rejects.toThrow("backend exploded");

    expect(store.isLoading).toBe(false);
  });
});
