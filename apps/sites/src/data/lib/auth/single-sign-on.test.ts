// @vitest-environment jsdom
import { privateKeyToAccount } from "viem/accounts";
import { afterEach, describe, expect, it, vi } from "vitest";

import { clearShared, freshestShared, fromShared, storeShared, toShared } from "./single-sign-on";
import type { AuthIdentity } from "./types";

const SIGNER = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const OTHER = "0x1111111111111111111111111111111111111111";
const KEY = `0x${"12".repeat(32)}` as const;
const EPHEMERAL = privateKeyToAccount(KEY).address.toLowerCase();
const PUB = privateKeyToAccount(KEY).publicKey;

function identity(signer = SIGNER, daysLeft = 30): AuthIdentity {
  const expiration = new Date(Date.now() + daysLeft * 86_400_000).toISOString();
  return {
    signer,
    ephemeral: { address: EPHEMERAL, privateKey: KEY, publicKey: PUB },
    expiration,
    authChain: [
      { type: "SIGNER", payload: signer, signature: "" },
      { type: "ECDSA_EPHEMERAL", payload: `Decentraland Login\nEphemeral address: ${EPHEMERAL}\nExpiration: ${expiration}`, signature: "0xsig" },
    ],
  };
}

afterEach(() => localStorage.clear());

describe("the session decentraland.org's dapps share", () => {
  it("is written under the account's lowercased key in @dcl/crypto's shape, public key included", () => {
    storeShared(identity("0x4E9C4A2502FDF71E93ED8ED6CA9DDBD891D6F295".toLowerCase()));
    const stored = JSON.parse(localStorage.getItem(`single-sign-on-${SIGNER}`)!);
    expect(Object.keys(stored).sort()).toEqual(["authChain", "ephemeralIdentity", "expiration"]);
    expect(stored.ephemeralIdentity).toEqual({ address: EPHEMERAL, publicKey: privateKeyToAccount(KEY).publicKey, privateKey: KEY });
    expect(stored.ephemeralIdentity.publicKey).toMatch(/^0x04[0-9a-f]{128}$/);
  });

  it("reads back as the same session", () => {
    const original = identity();
    expect(fromShared(toShared(original, PUB))).toEqual(original);
  });

  it("skips an expired, malformed or misfiled entry", () => {
    expect(fromShared(toShared(identity(SIGNER, -1), PUB))).toBeNull();
    expect(fromShared({ ...toShared(identity(), PUB), authChain: [] })).toBeNull();
    expect(fromShared({ ...toShared(identity(), PUB), ephemeralIdentity: { address: EPHEMERAL, publicKey: "0x04" } })).toBeNull();
    expect(fromShared("nonsense")).toBeNull();
    storeShared(identity(SIGNER, -1));
    expect(localStorage.length).toBe(0);
    // An entry filed under another account's key is not that account's session.
    localStorage.setItem(`single-sign-on-${OTHER}`, JSON.stringify(toShared(identity(SIGNER), PUB)));
    expect(freshestShared()).toBeNull();
  });

  it("takes the account signed in last when several are stored", () => {
    storeShared(identity(SIGNER, 5));
    storeShared(identity(OTHER, 20));
    localStorage.setItem("single-sign-on-0xnot-an-address", "{}");
    expect(freshestShared()?.signer).toBe(OTHER);
    clearShared(OTHER);
    expect(freshestShared()?.signer).toBe(SIGNER);
  });

  it("derives the public key for a session kept before it was recorded", async () => {
    const { publicKey: _, ...ephemeral } = identity().ephemeral;
    storeShared({ ...identity(), ephemeral });
    await vi.waitFor(() => expect(localStorage.getItem(`single-sign-on-${SIGNER}`)).not.toBeNull());
    expect(JSON.parse(localStorage.getItem(`single-sign-on-${SIGNER}`)!).ephemeralIdentity.publicKey).toBe(PUB);
  });

  it("never writes a session back after a sign-out that came while its key was derived", async () => {
    const { publicKey: _, ...ephemeral } = identity().ephemeral;
    storeShared({ ...identity(), ephemeral });
    clearShared(SIGNER);
    await import("viem/accounts");
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(localStorage.getItem(`single-sign-on-${SIGNER}`)).toBeNull();
  });
});
