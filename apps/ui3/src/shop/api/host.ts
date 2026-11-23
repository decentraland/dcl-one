// The browser host's wiring for the Shop API. The client (@shop/api/client, @shop/api/shop) reads no configuration and
// reaches for no network or identity of its own; this module gives it each of them:
// - where the API is: the configured mode and base, in their precedence, the base normalized (data/catalyst);
// - how a request travels: fetch, or the Explorer's engine bridge;
// - who signs: a web identity, the identity ui3 keeps in localStorage, or the engine (the bridge pair);
// - the document's client, signed with the stored identity, which a host that owns the session replaces.

import { fetchTransport, identitySigner, shopApiMode } from "@web-core/auth/shopRequest";
import { getDocumentShopClient, setShopClient } from "@shop/api/document-client";

export { fetchTransport, identitySigner, setShopClient, shopApiMode };
import { catalystBase, signedFetch } from "../../data/catalyst/client";
import type { ShopPeers, UrnNetwork } from "@shop/detail/lib/urn";
import type { ShopApiOptions } from "@shop/api/client";
import { createShopClient, type ShopClient } from "@shop/api/shop";
import type { ShopSigner, ShopTransport } from "@shop/api/signer";

type ShopWindow = Window &
  typeof globalThis & { __SHOP_PEERS__?: Partial<Record<UrnNetwork, string>> };

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

// A host that owns the session can install its client before the default is needed.
export function shopClient(): ShopClient {
  return getDocumentShopClient(() => createShopClient(hostApiOptions({ signer: () => storedIdentitySigner() })));
}
