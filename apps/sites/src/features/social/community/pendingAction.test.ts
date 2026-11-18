// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";

import { PENDING_ACTION_TTL_MS, rememberAction, takeAction } from "./pendingAction";

const ID = "e99471aa-31c4-4952-abf6-99905445f43b";
const NOW = 1_800_000_000_000;

afterEach(() => sessionStorage.clear());

describe("pending community action", () => {
  it("accepts the matching marker once", () => {
    const nonce = rememberAction(ID, "join", sessionStorage, NOW);
    expect(takeAction(ID, "join", nonce, sessionStorage, NOW + 1000)).toBe(true);
    expect(takeAction(ID, "join", nonce, sessionStorage, NOW + 2000)).toBe(false);
  });

  it("refuses a missing nonce, a different nonce, action or community, and consumes the marker", () => {
    const nonce = rememberAction(ID, "join", sessionStorage, NOW);
    expect(takeAction(ID, "join", null, sessionStorage, NOW)).toBe(false);
    expect(takeAction(ID, "join", "other", sessionStorage, NOW)).toBe(false);
    expect(sessionStorage.length).toBe(0);
    rememberAction(ID, "join", sessionStorage, NOW);
    expect(takeAction(ID, "requestToJoin", nonce, sessionStorage, NOW)).toBe(false);
    const again = rememberAction(ID, "join", sessionStorage, NOW);
    expect(takeAction("another", "join", again, sessionStorage, NOW)).toBe(false);
  });

  it("refuses an expired marker or one from the future", () => {
    const nonce = rememberAction(ID, "join", sessionStorage, NOW);
    expect(takeAction(ID, "join", nonce, sessionStorage, NOW + PENDING_ACTION_TTL_MS + 1)).toBe(false);
    const future = rememberAction(ID, "join", sessionStorage, NOW + 60_000);
    expect(takeAction(ID, "join", future, sessionStorage, NOW)).toBe(false);
  });

  it("refuses a corrupted marker", () => {
    sessionStorage.setItem("dcl:community-action", "{not json");
    expect(takeAction(ID, "join", "n", sessionStorage, NOW)).toBe(false);
  });

  it("records and accepts nothing without storage", () => {
    expect(rememberAction(ID, "join", null, NOW)).toBeNull();
    expect(takeAction(ID, "join", "n", null, NOW)).toBe(false);
  });

  it("records nothing when writing throws", () => {
    const throwing = { setItem: () => { throw new Error("QuotaExceededError"); } } as unknown as Storage;
    expect(rememberAction(ID, "join", throwing, NOW)).toBeNull();
  });
});
