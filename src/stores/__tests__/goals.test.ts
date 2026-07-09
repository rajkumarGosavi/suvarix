import { createPinia, setActivePinia } from "pinia";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { useGoalsStore } from "@/stores/goals";

const carGoal = {
  id: 1,
  name: "Car",
  category: "vehicle",
  targetAmount: 800000,
  targetDate: "2027-06-01",
  notes: null,
  achievedAt: null,
  createdAt: "2026-01-01",
  updatedAt: "2026-01-01",
};

describe("useGoalsStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  afterEach(() => {
    clearMocks();
  });

  it("fetchGoals stores the list and toggles loading", async () => {
    mockIPC((cmd) => {
      if (cmd === "list_goals") return [carGoal];
      throw new Error(`unexpected command: ${cmd}`);
    });

    const store = useGoalsStore();
    const pending = store.fetchGoals();
    expect(store.loading).toBe(true);
    await pending;

    expect(store.loading).toBe(false);
    expect(store.goals).toEqual([carGoal]);
  });

  it("fetchGoals resets loading when the backend rejects", async () => {
    mockIPC(() => {
      throw new Error("backend exploded");
    });

    const store = useGoalsStore();
    await expect(store.fetchGoals()).rejects.toThrow("backend exploded");
    expect(store.loading).toBe(false);
  });

  it("addGoal sends the payload then re-fetches", async () => {
    const calls: Array<{ cmd: string; args: any }> = [];
    mockIPC((cmd, args) => {
      calls.push({ cmd, args });
      if (cmd === "list_goals") return [carGoal];
      if (cmd === "add_goal") return 1;
      throw new Error(`unexpected command: ${cmd}`);
    });

    const store = useGoalsStore();
    const payload = {
      name: "Car",
      category: "vehicle",
      targetAmount: 800000,
      targetDate: "2027-06-01",
      notes: null,
    };
    await store.addGoal(payload);

    expect(calls.find((c) => c.cmd === "add_goal")?.args).toEqual({ payload });
    expect(store.goals).toEqual([carGoal]);
  });

  it("markAchieved invokes mark_goal_achieved by id then re-fetches", async () => {
    const achieved = { ...carGoal, achievedAt: "2026-07-09" };
    const calls: Array<{ cmd: string; args: any }> = [];
    mockIPC((cmd, args) => {
      calls.push({ cmd, args });
      if (cmd === "list_goals") return [achieved];
      if (cmd === "mark_goal_achieved") return null;
      throw new Error(`unexpected command: ${cmd}`);
    });

    const store = useGoalsStore();
    await store.markAchieved(1);

    expect(calls.find((c) => c.cmd === "mark_goal_achieved")?.args).toEqual({ id: 1 });
    expect(store.goals[0].achievedAt).toBe("2026-07-09");
  });

  it("deleteGoal invokes delete_goal by id then re-fetches", async () => {
    const calls: Array<{ cmd: string; args: any }> = [];
    mockIPC((cmd, args) => {
      calls.push({ cmd, args });
      if (cmd === "list_goals") return [];
      if (cmd === "delete_goal") return null;
      throw new Error(`unexpected command: ${cmd}`);
    });

    const store = useGoalsStore();
    await store.deleteGoal(1);

    expect(calls.find((c) => c.cmd === "delete_goal")?.args).toEqual({ id: 1 });
    expect(store.goals).toEqual([]);
  });
});
