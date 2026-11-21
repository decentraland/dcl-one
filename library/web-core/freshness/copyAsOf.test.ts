import { describe, expect, it } from "vitest";
import { olderCopy } from "./copyAsOf";

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
