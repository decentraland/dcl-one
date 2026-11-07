import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { recoverMessageAddress } from "viem";
import { sessionFor, signedHeaders, walletKey, WALLET_KEY_ENV } from "./session.mts";

// An obviously fake key: the tests never touch a real wallet.
const KEY = `0x${"11".repeat(32)}` as const;
const NOW = Date.parse("2026-11-06T12:00:00Z");

describe("the signed-in session", () => {
  it("is the same for the same wallet and clock, so captures do not change with it", async () => {
    const a = await sessionFor(KEY, NOW);
    const b = await sessionFor(KEY, NOW);
    assert.deepEqual(a, b);
    assert.equal(a.identity.expiration, "2026-11-07T12:00:00.000Z", "valid for a day");
  });

  it("is signed by the wallet for its ephemeral key, as @dcl/single-sign-on-client keeps it", async () => {
    const { address, identity } = await sessionFor(KEY, NOW);
    const [signer, ephemeral] = identity.authChain;
    assert.equal(signer.type, "SIGNER");
    assert.equal(signer.payload, address);
    assert.equal(ephemeral.type, "ECDSA_EPHEMERAL");
    assert.match(ephemeral.payload, new RegExp(`Ephemeral address: ${identity.ephemeralIdentity.address}`));
    const recovered = await recoverMessageAddress({ message: ephemeral.payload, signature: ephemeral.signature as `0x${string}` });
    assert.equal(recovered.toLowerCase(), address);
  });

  it("signs a request as ADR-44 asks, by the ephemeral key, at the time given", async () => {
    const headers = await signedHeaders(KEY, "GET", "/users/0xabc/credits", NOW);
    assert.equal(headers["x-identity-timestamp"], String(NOW));
    const entity = JSON.parse(headers["x-identity-auth-chain-2"]);
    assert.equal(entity.type, "ECDSA_SIGNED_ENTITY");
    assert.equal(entity.payload, `get:/users/0xabc/credits:${NOW}:{}`);
    const { identity } = await sessionFor(KEY, NOW);
    const recovered = await recoverMessageAddress({ message: entity.payload, signature: entity.signature });
    assert.equal(recovered, identity.ephemeralIdentity.address);
  });

  it("refuses to run a signed-in page without the test wallet's key", () => {
    assert.throws(() => walletKey({}), new RegExp(WALLET_KEY_ENV));
    assert.throws(() => walletKey({ [WALLET_KEY_ENV]: "0x1234" }), new RegExp(WALLET_KEY_ENV));
    assert.equal(walletKey({ [WALLET_KEY_ENV]: ` ${KEY} ` }), KEY);
  });
});
