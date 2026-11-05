// A signed fetch (ADR-44) from a wallet key held in Node: the headers decentraland-crypto-fetch sends,
// so the journey can call the stack's signed routes (the mock card top-up) as the wallet itself. The
// key never leaves Node; a fresh ephemeral key signs the request and expires after five minutes.
import type { Hex } from "viem";
import { generatePrivateKey, privateKeyToAccount } from "viem/accounts";

export type AuthLink = { type: "SIGNER" | "ECDSA_EPHEMERAL" | "ECDSA_SIGNED_ENTITY"; payload: string; signature: string };

/** What the request signs: `method:path:timestamp:metadata`, lowercased (ADR-44). */
export function requestPayload(method: string, path: string, timestamp: string, metadata: string): string {
  return `${method}:${path}:${timestamp}:${metadata}`.toLowerCase();
}

/** The `x-identity-*` headers for one request to `path` (the URL's path, as the server sees it). */
export async function signedHeaders(key: Hex, method: string, path: string, now = Date.now()): Promise<Record<string, string>> {
  const wallet = privateKeyToAccount(key);
  const ephemeral = privateKeyToAccount(generatePrivateKey());
  const expiration = new Date(now + 5 * 60_000).toISOString();
  const login = ["Decentraland Login", `Ephemeral address: ${ephemeral.address.toLowerCase()}`, `Expiration: ${expiration}`].join("\n");
  const timestamp = String(now);
  const metadata = "{}";
  const payload = requestPayload(method, path, timestamp, metadata);
  const chain: AuthLink[] = [
    { type: "SIGNER", payload: wallet.address.toLowerCase(), signature: "" },
    { type: "ECDSA_EPHEMERAL", payload: login, signature: await wallet.signMessage({ message: login }) },
    { type: "ECDSA_SIGNED_ENTITY", payload, signature: await ephemeral.signMessage({ message: payload }) },
  ];
  const headers: Record<string, string> = { "x-identity-timestamp": timestamp, "x-identity-metadata": metadata };
  chain.forEach((link, i) => (headers[`x-identity-auth-chain-${i}`] = JSON.stringify(link)));
  return headers;
}

/** POSTs JSON to `url` signed as the key's wallet; throws with the server's answer unless it is 2xx. */
export async function signedPost<T>(key: Hex, url: string, body: unknown): Promise<T> {
  const headers = await signedHeaders(key, "post", new URL(url).pathname);
  const res = await fetch(url, { method: "POST", headers: { ...headers, "content-type": "application/json" }, body: JSON.stringify(body) });
  const text = await res.text();
  if (!res.ok) throw new Error(`POST ${new URL(url).pathname} answered ${res.status}: ${text.slice(0, 300)}`);
  return JSON.parse(text) as T;
}
