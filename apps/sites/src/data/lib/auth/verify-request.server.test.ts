import { generatePrivateKey, privateKeyToAccount } from "viem/accounts";
import { describe, expect, it } from "vitest";

import { createIdentityFromPrivateKey, generateEphemeralKey } from "./identity";
import { AUTH_CHAIN_HEADER_PREFIX, signRequest } from "./signer";
import { parseExpiration, verifyAuthChainRequest } from "./verify-request.server";

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

  it("accepts only a Decentraland Login delegation", async () => {
    const delegatedWith = async (firstLine: string) => {
      const account = privateKeyToAccount(generatePrivateKey());
      const ephemeral = generateEphemeralKey();
      const expiration = new Date(Date.now() + 60_000).toISOString();
      const message = [firstLine, `Ephemeral address: ${ephemeral.address}`, `Expiration: ${expiration}`].join("\n");
      const signer = account.address.toLowerCase();
      const identity = {
        signer,
        ephemeral,
        expiration,
        authChain: [
          { type: "SIGNER" as const, payload: signer, signature: "" },
          { type: "ECDSA_EPHEMERAL" as const, payload: message, signature: await account.signMessage({ message }) },
        ],
      };
      const s = await signRequest(identity, "GET", "/v1/collections", {});
      return verifyAuthChainRequest(new Request("http://localhost/v1/collections", { headers: s.headers }));
    };

    expect((await delegatedWith("Decentraland Login")).ok).toBe(true);
    for (const firstLine of ["Decentraland Storage Delegation", "Decentraland Login ", "decentraland login", ""]) {
      const res = await delegatedWith(firstLine);
      expect(res).toEqual({ ok: false, status: 401, error: "malformed ephemeral payload" });
    }
  });

  it("refuses a delegation with no purpose line", async () => {
    const res = await delegatedRequest(
      (address) => `Ephemeral address: ${address}\nExpiration: ${new Date(Date.now() + 60_000).toISOString()}`,
    );
    expect(res).toEqual({ ok: false, status: 401, error: "malformed ephemeral payload" });
  });

  it("refuses any link after the signed request link", async () => {
    const account = privateKeyToAccount(generatePrivateKey());
    const ephemeral = generateEphemeralKey();
    const expiration = new Date(Date.now() + 60_000).toISOString();
    const message = `Decentraland Login\nEphemeral address: ${ephemeral.address}\nExpiration: ${expiration}`;
    const signer = account.address.toLowerCase();
    const identity = {
      signer,
      ephemeral,
      expiration,
      authChain: [
        { type: "SIGNER" as const, payload: signer, signature: "" },
        { type: "ECDSA_EPHEMERAL" as const, payload: message, signature: await account.signMessage({ message }) },
      ],
    };
    const s = await signRequest(identity, "GET", "/v1/collections", {});
    const headers = new Headers(s.headers);
    const signedLink = JSON.parse(headers.get(`${AUTH_CHAIN_HEADER_PREFIX}2`)!);
    const trailing = new Headers(headers);
    trailing.set(`${AUTH_CHAIN_HEADER_PREFIX}3`, headers.get(`${AUTH_CHAIN_HEADER_PREFIX}1`)!);
    expect(signedLink.type).toBe("ECDSA_SIGNED_ENTITY");

    expect((await verifyAuthChainRequest(new Request("http://localhost/v1/collections", { headers }))).ok).toBe(true);
    const res = await verifyAuthChainRequest(new Request("http://localhost/v1/collections", { headers: trailing }));
    expect(res).toEqual({ ok: false, status: 401, error: "ECDSA_SIGNED_ENTITY at link 2 must be the last link" });
  });

  it("requires the ephemeral address on line 2 and the expiration on line 3", async () => {
    const account = privateKeyToAccount(generatePrivateKey());
    const ephemeral = generateEphemeralKey();
    const expiration = new Date(Date.now() + 60_000).toISOString();
    const message = `Decentraland Login\nExpiration: ${expiration}\nEphemeral address: ${ephemeral.address}`;
    const signer = account.address.toLowerCase();
    const s = await signRequest(
      {
        signer,
        ephemeral,
        expiration,
        authChain: [
          { type: "SIGNER", payload: signer, signature: "" },
          { type: "ECDSA_EPHEMERAL", payload: message, signature: await account.signMessage({ message }) },
        ],
      },
      "GET",
      "/v1/collections",
      {},
    );
    const res = await verifyAuthChainRequest(new Request("http://localhost/v1/collections", { headers: s.headers }));
    expect(res).toEqual({ ok: false, status: 401, error: "malformed ephemeral payload" });
  });

  /** A signed GET whose delegation payload is `payload(ephemeralAddress)`, the owner signing `signed(payload)`. */
  async function delegatedRequest(
    payload: (ephemeralAddress: string) => string,
    signed: (payload: string) => string = (text) => text,
  ) {
    const account = privateKeyToAccount(generatePrivateKey());
    const ephemeral = generateEphemeralKey();
    const message = payload(ephemeral.address);
    const signer = account.address.toLowerCase();
    const s = await signRequest(
      {
        signer,
        ephemeral,
        expiration: new Date(Date.now() + 60_000).toISOString(),
        authChain: [
          { type: "SIGNER", payload: signer, signature: "" },
          { type: "ECDSA_EPHEMERAL", payload: message, signature: await account.signMessage({ message: signed(message) }) },
        ],
      },
      "GET",
      "/v1/collections",
      {},
    );
    return verifyAuthChainRequest(new Request("http://localhost/v1/collections", { headers: s.headers }));
  }

  it("accepts an expiration Date.parse reads, such as a bare date", async () => {
    for (const expiration of ["2999-01-01", "Jan 1 2999"]) {
      const res = await delegatedRequest((address) => `Decentraland Login\nEphemeral address: ${address}\nExpiration: ${expiration}`);
      expect(res.ok, expiration).toBe(true);
    }
  });

  it("refuses an expiration it cannot read instead of treating it as never expiring", async () => {
    for (const expiration of ["not a date", "", "32503680000000", " 2999-01-01T00:00:00.000Z"]) {
      const res = await delegatedRequest((address) => `Decentraland Login\nEphemeral address: ${address}\nExpiration: ${expiration}`);
      expect(res, expiration).toEqual({ ok: false, status: 401, error: "malformed ephemeral payload" });
    }
  });

  it("verifies the delegation signature over the payload without carriage returns", async () => {
    const crlf = (address: string) =>
      `Decentraland Login\r\nEphemeral address: ${address}\r\nExpiration: ${new Date(Date.now() + 60_000).toISOString()}`;
    const stripped = (text: string) => text.replace(/\r/g, "");

    expect((await delegatedRequest(crlf, stripped)).ok).toBe(true);
    expect(await delegatedRequest(crlf)).toEqual({ ok: false, status: 401, error: "broken auth chain at link 1" });
  });
});

describe("parseExpiration", () => {
  it("reads the expiration as Date.parse does", () => {
    const accepted: Array<[string, number]> = [
      ["2100-01-01T00:00:00.000Z", 4102444800000],
      ["2100-01-01T00:00:00+03:00", 4102434000000],
      ["2026-11-19", 1795046400000],
      ["Thu, 13 Feb 2020 15:31:10 GMT", 1581607870000],
      ["Thu Feb 13 2020 15:31:10 GMT-0300 (Argentina Standard Time)", 1581618670000],
    ];
    for (const [raw, millis] of accepted) expect(parseExpiration(raw), raw).toBe(millis);
  });

  it("refuses what Date.parse cannot read, without trimming", () => {
    for (const raw of ["not a date", "", "1700000000000", "  2100-01-01T00:00:00.000Z  "]) {
      expect(parseExpiration(raw), raw).toBeNull();
    }
  });
});
