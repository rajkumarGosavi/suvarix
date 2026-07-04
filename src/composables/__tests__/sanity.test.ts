import { describe, expect, it } from "vitest";

describe("vitest sanity", () => {
  it("runs and resolves the @ alias-free basics", () => {
    expect(1 + 1).toBe(2);
  });
});
