import { generatePrivateKey, privateKeyToAccount } from "viem/accounts";
import { describe, expect, it } from "vitest";

import { createIdentityFromPrivateKey } from "./identity";
import { signRequest } from "./signer";
import { verifyAuthChainRequest } from "./verify-request.server";

async function signed(
  key: `0x${string}`,
  pathname: string,
  identityOpts: { expirationMs?: number } = {},
): Promise<{ request: Request; wallet: string }> {
  const identity = await createIdentityFromPrivateKey(key, identityOpts);
  const s = await signRequest(identity, "GET", pathname, {});
  return {
    request: new Request(`http://localhost${pathname}`, { headers: new Headers(s.headers) }),
    wallet: identity.signer,
  };
}

describe("verifyAuthChainRequest", () => {
  it("verifies a real signed request, derives the wallet from the chain, and rejects an unsigned one with 401", async () => {
    const key = generatePrivateKey();
    const { request, wallet } = await signed(key, "/v1/collections");
    const res = await verifyAuthChainRequest(request);
    expect(res.ok).toBe(true);
    if (!res.ok) return;
    expect(res.wallet).toBe(wallet);
    expect(res.wallet).toBe(privateKeyToAccount(key).address.toLowerCase());

    const anon = await verifyAuthChainRequest(new Request("http://localhost/v1/collections"));
    expect(anon.ok).toBe(false);
    if (anon.ok) return;
    expect(anon.status).toBe(401);
  });

  it("rejects an expired ephemeral identity, a stale timestamp, and a signature replayed across paths with 401", async () => {
    const expired = await signed(generatePrivateKey(), "/v1/collections", { expirationMs: 1_000 });
    const expiredTs = Number(expired.request.headers.get("x-identity-timestamp"));
    const expiredRes = await verifyAuthChainRequest(expired.request, { now: expiredTs + 2_000 });
    expect(expiredRes.ok).toBe(false);
    if (expiredRes.ok) return;
    expect(expiredRes.status).toBe(401);
    expect(expiredRes.error).toContain("expired");

    const stale = await signed(generatePrivateKey(), "/v1/collections");
    const staleTs = Number(stale.request.headers.get("x-identity-timestamp"));
    const staleRes = await verifyAuthChainRequest(stale.request, { now: staleTs + 20 * 60_000 });
    expect(staleRes.ok).toBe(false);
    if (staleRes.ok) return;
    expect(staleRes.status).toBe(401);

    const { request } = await signed(generatePrivateKey(), "/v1/items/one");
    const moved = new Request("http://localhost/v1/items/two", { headers: request.headers });
    const replayRes = await verifyAuthChainRequest(moved);
    expect(replayRes.ok).toBe(false);
    if (replayRes.ok) return;
    expect(replayRes.status).toBe(401);
  });
});
