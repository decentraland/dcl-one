// Signing through ui3's existing seams: data/auth/signerCore (the ADR-44 auth-chain headers, the same
// code sites' signer wraps) for a web identity, and data/catalyst/client `signedFetch` for the Explorer,
// where the engine signs and sends the request itself.

import type { AuthIdentity } from "../../data/auth/identity";
import { signAuthChain } from "../../data/auth/signerCore";
import { signedFetch } from "../../data/catalyst/client";

export type ShopSigner = {
  /** Lowercase address the requests are signed for. */
  readonly address: string;
  /** Auth-chain headers for `method` + `path` (the exact pathname requested, no query). */
  sign(method: string, path: string, metadata: Record<string, unknown>): Promise<Record<string, string>>;
};

export type TransportRequest = {
  url: string;
  method: string;
  headers: Record<string, string>;
  body?: string;
  signal?: AbortSignal;
  signed: boolean;
  signPath: string;
};

export type ShopTransport = (req: TransportRequest) => Promise<{ status: number; body: string }>;

function expired(expiration: string, now: number): boolean {
  const at = Date.parse(expiration);
  return !Number.isFinite(at) || at <= now;
}

/** Signs with a web identity (sites' session, or the host's identity in the native contract). */
export function identitySigner(identity: AuthIdentity | null, now: () => number = Date.now): ShopSigner | null {
  if (!identity || expired(identity.expiration, now())) return null;
  return {
    address: identity.signer.toLowerCase(),
    async sign(method, path, metadata) {
      if (expired(identity.expiration, now())) throw new Error("The sign-in has expired. Sign in again to continue.");
      const { headers } = await signAuthChain(
        identity.ephemeral.privateKey,
        identity.authChain,
        method,
        path,
        String(now()),
        JSON.stringify(metadata),
      );
      return headers;
    },
  };
}

/**
 * Signs with the identity ui3 keeps in localStorage (the Explorer's engine login). Loaded lazily: the
 * reader validates through the engine bridge's schemas, which a web page does not need up front.
 */
export async function storedIdentitySigner(now: () => number = Date.now): Promise<ShopSigner | null> {
  const [{ loadStoredIdentity }, { identitySigner: storedSigner }] = await Promise.all([
    import("../../data/auth/signedFetchLocal"),
    import("../../data/auth/engineLogin"),
  ]);
  const stored = loadStoredIdentity(now());
  const address = stored ? storedSigner(stored) : null;
  if (!stored || !address) return null;
  return identitySigner(
    {
      signer: address,
      ephemeral: { address: stored.ephemeralIdentity.address, privateKey: stored.ephemeralIdentity.privateKey as `0x${string}` },
      expiration: stored.expiration,
      authChain: stored.authChain,
    },
    now,
  );
}

/**
 * The Explorer transport: signed requests go through the engine bridge, which signs them for the
 * engine's own account; unsigned ones use fetch. Use it with `bridgeSigner(address)`:
 * `createShopApi({ transport: bridgeTransport(), signer: bridgeSigner(address) })`.
 */
export function bridgeTransport(fetchImpl: typeof fetch = (...a) => fetch(...a)): ShopTransport {
  return async (req) => {
    if (!req.signed) {
      const res = await fetchImpl(req.url, { method: req.method, headers: req.headers, body: req.body, signal: req.signal });
      return { status: res.status, body: res.status === 204 ? "" : await res.text() };
    }
    return signedFetch(req.url, { method: req.method, body: req.body });
  };
}

/** For the Explorer: a signer that only marks requests as signed; the bridge does the signing. */
export function bridgeSigner(address: string): ShopSigner {
  return { address: address.toLowerCase(), sign: async () => ({}) };
}
