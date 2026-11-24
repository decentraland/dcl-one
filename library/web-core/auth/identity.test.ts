import { afterEach, describe, expect, it, vi } from "vitest";
import { privateKeyToAccount } from "viem/accounts";

import {
  buildEphemeralMessage,
  createIdentityWith,
  DEFAULT_EXPIRATION_MS,
  generateEphemeralKey,
  IDENTITY_STORAGE_KEY,
  toStoredIdentity,
} from "./identity";

afterEach(() => vi.useRealTimers());

describe("buildEphemeralMessage", () => {
  it("is the three-line Decentraland login message", () => {
    expect(buildEphemeralMessage("0xabc", new Date(Date.UTC(2026, 0, 2, 3, 4, 5)))).toBe(
      "Decentraland Login\nEphemeral address: 0xabc\nExpiration: 2026-01-02T03:04:05.000Z",
    );
  });
});

describe("generateEphemeralKey", () => {
  it("returns a fresh key and its lower-cased address", () => {
    const key = generateEphemeralKey();
    expect(key.address).toBe(privateKeyToAccount(key.privateKey).address.toLowerCase());
    expect(generateEphemeralKey().privateKey).not.toBe(key.privateKey);
  });
});

describe("createIdentityWith", () => {
  it("signs the ephemeral message once and builds the SIGNER, ECDSA_EPHEMERAL chain, expiring in a week by default", async () => {
    vi.useFakeTimers({ now: Date.UTC(2026, 0, 1) });
    const signMessage = vi.fn(async (message: string) => `sig:${message.length}`);
    const identity = await createIdentityWith("0xABCDEF", signMessage);

    expect(DEFAULT_EXPIRATION_MS).toBe(7 * 24 * 60 * 60 * 1000);
    expect(identity.expiration).toBe(new Date(Date.UTC(2026, 0, 8)).toISOString());
    expect(identity.signer).toBe("0xabcdef");
    const message = buildEphemeralMessage(identity.ephemeral.address, new Date(identity.expiration));
    expect(signMessage).toHaveBeenCalledTimes(1);
    expect(signMessage).toHaveBeenCalledWith(message);
    expect(identity.authChain).toEqual([
      { type: "SIGNER", payload: "0xABCDEF", signature: "" },
      { type: "ECDSA_EPHEMERAL", payload: message, signature: `sig:${message.length}` },
    ]);
  });

  it("honours a custom expiration", async () => {
    vi.useFakeTimers({ now: Date.UTC(2026, 0, 1) });
    const identity = await createIdentityWith("0x1", async () => "s", { expirationMs: 1000 });
    expect(identity.expiration).toBe("2026-01-01T00:00:01.000Z");
  });

  it("propagates a refused signature", async () => {
    await expect(createIdentityWith("0x1", async () => Promise.reject(new Error("denied")))).rejects.toThrow("denied");
  });
});

describe("toStoredIdentity", () => {
  it("keeps the ephemeral key, expiration and chain under the stored shape", async () => {
    const identity = await createIdentityWith("0x1", async () => "s");
    expect(IDENTITY_STORAGE_KEY).toBe("dcl-auth-identity");
    expect(toStoredIdentity(identity)).toEqual({
      ephemeralIdentity: { address: identity.ephemeral.address, privateKey: identity.ephemeral.privateKey },
      expiration: identity.expiration,
      authChain: identity.authChain,
    });
  });
});
