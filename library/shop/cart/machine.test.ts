import { describe, expect, it } from "vitest";

import type { PurchaseState } from "../model/purchase";
import { nameRegistration, parcelResale, storeMint, web3 } from "../payment/fixtures";
import {
  applyEvent,
  canTransition,
  checkoutSummary,
  InvalidTransition,
  newCheckout,
  prepareResume,
  resumePlan,
  type GroupEvent,
} from "./machine";
import { partitionCart, type PricedLine } from "./partition";

function checkout(): PurchaseState {
  const s = web3();
  const lines: PricedLine[] = [storeMint(s), parcelResale(s), nameRegistration(s)].map((offer, i) => ({
    line: { id: `l${i}`, asset: offer.asset, quantity: 1, method: null, listing: {}, display: { name: "x" }, provenance: {}, seen: null, addedAt: 0 },
    offer,
  }));
  return newCheckout("0xABC", partitionCart(lines, s, { report: () => {} }).groups, 1, "c1");
}

function run(state: PurchaseState, keyId: string, events: GroupEvent[]): PurchaseState {
  return events.reduce((s, e, i) => applyEvent(s, keyId, e, 10 + i), state);
}

describe("the group state machine", () => {
  it("each group advances alone: idle -> authorizing -> submitted -> pending -> confirmed", () => {
    let state = checkout();
    const [credits, mana, name] = state.groups.map((g) => g.keyId) as [string, string, string];
    expect(state.account).toBe("0xabc");
    state = run(state, credits, [
      { type: "authorize" },
      { type: "authorized", creditIds: ["0xa1"] },
      { type: "submit" },
      { type: "hash", txHash: "0xd4" },
      { type: "observed" },
      { type: "confirmed" },
    ]);
    state = run(state, mana, [{ type: "authorize" }, { type: "failed", failure: { kind: "rejected", message: "user rejected" } }]);
    state = run(state, name, [{ type: "authorize" }, { type: "authorized", creditIds: ["0xb2"] }, { type: "submit" }]);
    expect(state.groups.map((g) => g.status)).toEqual(["confirmed", "failed", "submitted"]);
    expect(state.groups[0]).toMatchObject({ txHash: "0xd4", creditIds: ["0xa1"], attempts: 1 });
    expect(checkoutSummary(state)).toMatchObject({ confirmed: 1, failed: 1, submitted: 1, complete: false, settled: 1, total: 3 });
  });

  it("resumes only unfinished groups: confirmed are left alone, handed-off ones are watched, never re-run", () => {
    let state = checkout();
    const [credits, mana, name] = state.groups.map((g) => g.keyId) as [string, string, string];
    state = run(state, credits, [{ type: "authorize" }, { type: "submit" }, { type: "confirmed", txHash: "0x1" }]);
    state = run(state, mana, [{ type: "authorize" }, { type: "submit" }]);
    state = run(state, name, [{ type: "authorize" }, { type: "authorized", creditIds: ["0xb2"] }]);
    const plan = resumePlan(state);
    expect(plan.done.map((g) => g.keyId)).toEqual([credits]);
    expect(plan.watch.map((g) => g.keyId)).toEqual([mana]);
    expect(plan.run.map((g) => g.keyId)).toEqual([name]);
    expect(plan.release, "a credit authorized for a group nothing was handed off for is cancelled first").toEqual(["0xb2"]);
    const resumed = run(prepareResume(state, 50), name, [{ type: "authorize" }]);
    expect(resumed.groups[2]).toMatchObject({ status: "authorizing", attempts: 2, creditIds: [] });
    expect(resumed.groups[0]!.status).toBe("confirmed");
  });

  it("a handed-off transaction is never failed on a guess", () => {
    let state = checkout();
    const key = state.groups[0]!.keyId;
    state = run(state, key, [{ type: "authorize" }, { type: "submit" }, { type: "hash", txHash: "0xd4" }]);
    const group = state.groups[0]!;
    expect(canTransition(group, { type: "failed", failure: { kind: "error", message: "timeout" } })).toBe(false);
    expect(canTransition(group, { type: "failed", failure: { kind: "rejected", message: "late" } })).toBe(false);
    expect(canTransition(group, { type: "failed", failure: { kind: "reverted", message: "reverted" } })).toBe(true);
    expect(canTransition(group, { type: "authorize" }), "no refund-and-retry double spend").toBe(false);
    expect(() => applyEvent(state, key, { type: "expired" })).toThrow(InvalidTransition);
  });

  it("a wallet rejection before any hash fails the group; an expired authorization can start again", () => {
    let state = checkout();
    const key = state.groups[0]!.keyId;
    state = run(state, key, [{ type: "authorize" }, { type: "submit" }, { type: "failed", failure: { kind: "rejected", message: "denied" } }]);
    expect(state.groups[0]!.status).toBe("failed");
    state = run(state, key, [{ type: "authorize" }, { type: "expired" }, { type: "authorize" }]);
    expect(state.groups[0]).toMatchObject({ status: "authorizing", attempts: 3, failure: null });
  });
});
