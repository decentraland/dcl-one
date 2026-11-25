import { afterEach, describe, expect, it } from "vitest";
import { copiesVersion, copyAsOf, noteCopy, olderCopy, resetCopies, subscribeCopies } from "./copyAsOf";

const NOW = Date.parse("2026-10-02T12:00:00Z");
const HOUR = 3_600_000;

describe("olderCopy", () => {
  it("a read over two copies is as old as the older and stale as soon as either is", () => {
    const fresh = { asOf: NOW - HOUR, staleAfterMs: 2 * HOUR };
    const older = { asOf: NOW - 3 * HOUR, staleAfterMs: 48 * HOUR };
    expect(olderCopy(fresh, older)).toEqual({ asOf: NOW - 3 * HOUR, staleAfterMs: 4 * HOUR });
    expect(olderCopy(null, fresh)).toEqual(fresh);
    expect(olderCopy(undefined, null)).toBeNull();
  });
});

describe("the noted copies", () => {
  afterEach(() => resetCopies());
  const body = (asOf: string, staleAfterSecs = 60) => ({ as_of: asOf, stale_after_secs: staleAfterSecs, data: [] });

  it("notifies on a new stamp only, and stops after unsubscribing", () => {
    let calls = 0;
    const unsubscribe = subscribeCopies(() => { calls += 1; });
    const before = copiesVersion();
    noteCopy("places", body("2026-10-02T11:00:00Z"));
    noteCopy("places", body("2026-10-02T11:00:00Z"));
    expect(calls, "the same stamp again is not a change").toBe(1);
    expect(copiesVersion()).toBe(before + 1);
    expect(copyAsOf(["places"])).toEqual({ asOf: Date.parse("2026-10-02T11:00:00Z"), staleAfterMs: 60_000 });
    unsubscribe();
    noteCopy("places", body("2026-10-02T11:30:00Z"));
    expect(calls).toBe(1);
  });

  it("forgets a domain whose reply carries no readable stamp, and resets to nothing", () => {
    noteCopy("events", body("2026-10-02T11:00:00Z"));
    noteCopy("events", { as_of: "not a date" });
    expect(copyAsOf(["events"])).toBeNull();
    noteCopy("worlds", body("2026-10-02T11:00:00Z"));
    let calls = 0;
    const unsubscribe = subscribeCopies(() => { calls += 1; });
    resetCopies();
    unsubscribe();
    expect(calls).toBe(1);
    expect(copyAsOf(["worlds", "events"])).toBeNull();
  });
});
