import { describe, expect, it } from "vitest";
import { recoverMessageAddress } from "viem";
import { privateKeyToAccount } from "viem/accounts";

import type { AuthLink } from "./identity";
import {
  AUTH_CHAIN_HEADER_PREFIX,
  AUTH_METADATA_HEADER,
  AUTH_TIMESTAMP_HEADER,
  buildRequestPayload,
  signAuthChain,
} from "./signerCore";

// A throwaway test key; it holds nothing on any chain.
const EPHEMERAL_KEY = `0x${"11".repeat(32)}` as const;

const chain: AuthLink[] = [
  { type: "SIGNER", payload: "0xsigner", signature: "" },
  { type: "ECDSA_EPHEMERAL", payload: "Decentraland Login", signature: "0xeph" },
];

describe("buildRequestPayload", () => {
  it("joins method, path, timestamp and metadata with colons, lower-cased", () => {
    expect(buildRequestPayload("POST", "/Shop-API/V1/Cart", "1700000000000", '{"Signer":"X"}')).toBe(
      'post:/shop-api/v1/cart:1700000000000:{"signer":"x"}',
    );
  });
});

describe("signAuthChain", () => {
  it("appends the request signature to the chain and sends every link as a numbered header", async () => {
    const signed = await signAuthChain(EPHEMERAL_KEY, chain, "GET", "/a/B", "42", "{}");

    expect(signed.payload).toBe("get:/a/b:42:{}");
    expect(signed.authChain).toHaveLength(3);
    expect(signed.authChain.slice(0, 2)).toEqual(chain);
    const last = signed.authChain[2]!;
    expect(last.type).toBe("ECDSA_SIGNED_ENTITY");
    expect(last.payload).toBe(signed.payload);
    await expect(recoverMessageAddress({ message: signed.payload, signature: last.signature as `0x${string}` })).resolves.toBe(
      privateKeyToAccount(EPHEMERAL_KEY).address,
    );

    expect(signed.headers).toEqual({
      [AUTH_TIMESTAMP_HEADER]: "42",
      [AUTH_METADATA_HEADER]: "{}",
      [`${AUTH_CHAIN_HEADER_PREFIX}0`]: JSON.stringify(chain[0]),
      [`${AUTH_CHAIN_HEADER_PREFIX}1`]: JSON.stringify(chain[1]),
      [`${AUTH_CHAIN_HEADER_PREFIX}2`]: JSON.stringify(last),
    });
    expect(AUTH_CHAIN_HEADER_PREFIX).toBe("x-identity-auth-chain-");
    expect(AUTH_TIMESTAMP_HEADER).toBe("x-identity-timestamp");
    expect(AUTH_METADATA_HEADER).toBe("x-identity-metadata");
  });

  it("leaves the caller's chain untouched", async () => {
    const before = structuredClone(chain);
    await signAuthChain(EPHEMERAL_KEY, chain, "GET", "/", "1", "");
    expect(chain).toEqual(before);
  });

  it("rejects a malformed ephemeral key", async () => {
    await expect(signAuthChain("0x12", chain, "GET", "/", "1", "")).rejects.toThrow();
  });
});
