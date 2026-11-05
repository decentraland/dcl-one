import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { recoverMessageAddress } from "viem";
import { privateKeyToAccount } from "viem/accounts";
import { requestPayload, signedHeaders, type AuthLink } from "./signed.mts";

// A throwaway key with no funds anywhere, used only to check signatures.
const KEY = `0x${"11".repeat(32)}` as const;

describe("signedHeaders", () => {
  it("chains the wallet to an ephemeral key that signs this request", async () => {
    const now = 1_760_000_000_000;
    const h = await signedHeaders(KEY, "post", "/credits/topup/mock-card", now);
    const chain = [0, 1, 2].map((i) => JSON.parse(h[`x-identity-auth-chain-${i}`]!) as AuthLink);
    const wallet = privateKeyToAccount(KEY).address.toLowerCase();
    assert.deepEqual(chain.map((l) => l.type), ["SIGNER", "ECDSA_EPHEMERAL", "ECDSA_SIGNED_ENTITY"]);
    assert.equal(chain[0]!.payload, wallet);
    assert.equal((await recoverMessageAddress({ message: chain[1]!.payload, signature: chain[1]!.signature as `0x${string}` })).toLowerCase(), wallet);
    const ephemeral = chain[1]!.payload.match(/Ephemeral address: (0x[0-9a-f]{40})/)![1];
    assert.equal(chain[2]!.payload, requestPayload("post", "/credits/topup/mock-card", String(now), "{}"));
    assert.equal((await recoverMessageAddress({ message: chain[2]!.payload, signature: chain[2]!.signature as `0x${string}` })).toLowerCase(), ephemeral);
    assert.equal(h["x-identity-timestamp"], String(now));
    assert.match(chain[1]!.payload, new RegExp(`Expiration: ${new Date(now + 5 * 60_000).toISOString()}`));
  });

  it("never puts the key in a header", async () => {
    const h = await signedHeaders(KEY, "post", "/x");
    assert.ok(!JSON.stringify(h).includes(KEY.slice(2)));
  });
});
