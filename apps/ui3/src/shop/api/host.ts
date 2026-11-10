// The browser host's wiring for the Shop API. The client (@shop/api/client, @shop/api/shop) reads no configuration and
// reaches for no network or identity of its own; this module gives it each of them:
// - where the API is: the configured mode and base, in their precedence, the base normalized (data/catalyst);
// - how a request travels: fetch, or the Explorer's engine bridge;
// - who signs: a web identity, the identity ui3 keeps in localStorage, or the engine (the bridge pair);
// - the document's client, signed with the stored identity, which a host that owns the session replaces.

import type { AuthIdentity } from "../../data/auth/identity";
import { signAuthChain } from "../../data/auth/signerCore";
import { catalystBase, signedFetch } from "../../data/catalyst/client";
import type { ShopPeers, UrnNetwork } from "@shop/detail/lib/urn";
import type { ShopApiMode, ShopApiOptions } from "@shop/api/client";
import { createShopClient, type ShopClient } from "@shop/api/shop";
import type { ShopSigner, ShopTransport } from "@shop/api/signer";

type ShopWindow = Window &
  typeof globalThis & { __SHOP_API_MODE__?: string; __SHOP_PEERS__?: Partial<Record<UrnNetwork, string>> };

/** The API mode: the override, else the page's `window.__SHOP_API_MODE__`, else `VITE_SHOP_API_MODE`; legacy only when one says so. */
export function shopApiMode(override?: ShopApiMode): ShopApiMode {
  if (override) return override;
  const fromWindow = typeof window !== "undefined" ? (window as ShopWindow).__SHOP_API_MODE__ : undefined;
  const fromEnv = typeof import.meta !== "undefined" ? (import.meta.env?.VITE_SHOP_API_MODE as string | undefined) : undefined;
  const value = fromWindow ?? fromEnv;
  return value === "legacy" ? "legacy" : "canonical";
}

/** A transport over fetch. */
export function fetchTransport(fetchImpl: typeof fetch = (...a) => fetch(...a)): ShopTransport {
  return async (req) => {
    const res = await fetchImpl(req.url, {
      method: req.method,
      headers: req.headers,
      body: req.body,
      signal: req.signal,
    });
    return { status: res.status, body: res.status === 204 ? "" : await res.text() };
  };
}

export type HostApiOptions = Partial<ShopApiOptions> & { fetchImpl?: typeof fetch };

/** The client's options with this host's defaults: the configured base (normalized) and mode, and fetch. */
export function hostApiOptions(opts: HostApiOptions = {}): ShopApiOptions {
  return {
    base: catalystBase(opts.base),
    mode: shopApiMode(opts.mode),
    signer: opts.signer,
    transport: opts.transport ?? fetchTransport(opts.fetchImpl),
  };
}

/** Where a URN list's content may be read: the page's `window.__SHOP_PEERS__` per network, else the node. */
export function shopPeers(): ShopPeers {
  const byNetwork = typeof window !== "undefined" ? (window as ShopWindow).__SHOP_PEERS__ : undefined;
  return { byNetwork: byNetwork ?? {}, base: catalystBase() };
}

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
 * The Explorer transport, one half of a pair with `bridgeSigner`: signed requests go through the engine
 * bridge, which signs them for the engine's own account; unsigned ones use fetch. `bridgeSigner` only marks a
 * request as signed, so either half with anything else does not sign:
 * `createShopApi({ ...hostApiOptions(), transport: bridgeTransport(), signer: bridgeSigner(address) })`.
 * A signed request hands the bridge only its method and body: it carries no headers and cannot be aborted.
 */
export function bridgeTransport(fetchImpl: typeof fetch = (...a) => fetch(...a)): ShopTransport {
  return async (req) => {
    if (!req.signed) return fetchTransport(fetchImpl)(req);
    return signedFetch(req.url, { method: req.method, body: req.body });
  };
}

/** For the Explorer: a signer that only marks requests as signed; `bridgeTransport` has the bridge sign them. */
export function bridgeSigner(address: string): ShopSigner {
  return { address: address.toLowerCase(), sign: async () => ({}) };
}

let shared: ShopClient | null = null;

/** The document's client: canonical paths (or the legacy flag), signed with ui3's stored identity. A browser
 *  document's only: a server request builds its own. */
export function shopClient(): ShopClient {
  shared ??= createShopClient(hostApiOptions({ signer: () => storedIdentitySigner() }));
  return shared;
}

/** Hosts that own the session (sites, the native contract) install their own client once. */
export function setShopClient(client: ShopClient | null): void {
  shared = client;
}
