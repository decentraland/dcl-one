// @vitest-environment jsdom
import { recoverMessageAddress } from "viem";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { getIdentity } from "../auth/session";

beforeEach(() => {
  vi.resetModules();
});
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("a signed-out Cast watcher", () => {
  it("keeps a valid signed identity in memory without touching the account or storage", async () => {
    const session = getIdentity();
    const setItem = vi.spyOn(Storage.prototype, "setItem");
    const removeItem = vi.spyOn(Storage.prototype, "removeItem");
    const { guestWatcherIdentity } = await import("./cast-guest");
    const [first, second] = await Promise.all([guestWatcherIdentity(), guestWatcherIdentity()]);
    expect(second).toBe(first);
    expect(first.signer).toMatch(/^0x[0-9a-f]{40}$/);
    expect(first.authChain.map((link) => link.type)).toEqual(["SIGNER", "ECDSA_EPHEMERAL"]);
    expect(first.authChain[0].payload).toBe(first.signer);
    expect(Date.parse(first.expiration)).toBeGreaterThan(Date.now());
    const link = first.authChain[1];
    const recovered = await recoverMessageAddress({ message: link.payload, signature: link.signature as `0x${string}` });
    expect(recovered.toLowerCase()).toBe(first.signer);
    expect(link.payload).toContain(first.expiration);
    expect(link.payload).toContain(first.ephemeral.address);
    expect(getIdentity()).toBe(session);
    expect(setItem).not.toHaveBeenCalled();
    expect(removeItem).not.toHaveBeenCalled();
  });

  it("reuses the signer until five minutes before expiry, then makes a new one", async () => {
    vi.useFakeTimers();
    const { guestWatcherIdentity } = await import("./cast-guest");
    const first = await guestWatcherIdentity();
    vi.setSystemTime(Date.parse(first.expiration) - 6 * 60_000);
    expect(await guestWatcherIdentity()).toBe(first);
    vi.setSystemTime(Date.parse(first.expiration) - 5 * 60_000);
    const [refreshed, concurrent] = await Promise.all([guestWatcherIdentity(), guestWatcherIdentity()]);
    expect(concurrent).toBe(refreshed);
    expect(refreshed.signer).not.toBe(first.signer);
    expect(Date.parse(refreshed.expiration)).toBe(Date.now() + 60 * 60_000);
    expect(await guestWatcherIdentity()).toBe(refreshed);
  });
});
