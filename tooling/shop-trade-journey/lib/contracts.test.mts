import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";
import { AMOY_CREDITS_MANAGER } from "./contracts.mts";

describe("AMOY_CREDITS_MANAGER", () => {
  it("is the manager the testnet profile names", () => {
    const dev = readFileSync(new URL("../../dev.py", import.meta.url), "utf8");
    const named = [...dev.matchAll(/'CREDITS_MANAGER_CONTRACT':\s*'(0x[0-9a-fA-F]{40})'/g)].map((m) => m[1]!.toLowerCase());
    assert.deepEqual(named, [AMOY_CREDITS_MANAGER]);
  });
});
