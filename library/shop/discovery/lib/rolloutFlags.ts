// from: shop@3a322cf9:app/src/lib/featureFlags.ts (getIsFeatureEnabled), shop@94584a5c:app/src/hooks/useCreatorSalesEnabled.ts
// The Shop's rollout switches: Decentraland's feature-flags file read under upstream's keys (`${app}-${feature}` in
// `dapps.json`, so turning one on turns it on in both Shops), and how a host's own declarations combine with it. A
// flag is off until the file says `true`. Reading the file goes through the transport a host passes; holding the
// answer (its age, the retry and backoff, a primed answer, late answers, the hooks) is the host's
// (ui3: discovery/lib/remoteFlags.ts).

export const FLAGS_SERVICE = "https://feature-flags.decentraland.org";
export const FLAGS_APP = "dapps";

export const REMOTE_FLAG = {
  /** Whether the Shop asks for personalised suggestions at all; the row still hides itself without a signal. */
  suggestedForYou: "shop-suggested-for-you",
  /** Creators' running sales: discounted prices, the Best Deals rail and the deals filter (shop@94584a5c useCreatorSalesEnabled). */
  creatorSales: "shop-creator-sales",
  /**
   * The Marketplace inside the Shop: resales, bids, LAND, NAME resale, listing and selling, an item's owners and
   * sales, favorites lists. Shown only with this on AND a web3 wallet session (market/access.tsx). That only decides
   * what is drawn: the lock is the node's shop service reading the same key and refusing those calls while it is
   * off, so the flag is turned on only where the node does.
   */
  marketplaceFeatures: "shop-marketplace-features",
  /**
   * The creator's store dashboard, My Store (shop@3a322cf9 useMyStoreAccess). Its variant may name accounts, and
   * then only they get in (seller/lib/featureFlags.ts).
   */
  myStore: "shop-my-store",
} as const;

export type RemoteFlag = (typeof REMOTE_FLAG)[keyof typeof REMOTE_FLAG];

/** The flags file as kept: each flag on only where it says `true`, and each enabled variant's string payload. */
export type RemoteFlagsAnswer = { flags: Record<string, boolean>; variants: Record<string, string> };

/**
 * The service's answer as upstream keeps it (shop@3a322cf9 fetchSnapshot): a flag on only where it says `true`, and a
 * variant's payload only when the variant is enabled and its value a string; the rest of the envelope is dropped.
 */
export function parseRemoteFlags(body: unknown): RemoteFlagsAnswer {
  const file = (body && typeof body === "object" ? body : {}) as {
    flags?: Record<string, unknown> | null;
    variants?: Record<string, { enabled?: unknown; payload?: { value?: unknown } | null } | null> | null;
  };
  const flags: Record<string, boolean> = {};
  for (const [key, value] of Object.entries(file.flags ?? {})) flags[key] = value === true;
  const variants: Record<string, string> = {};
  for (const [key, variant] of Object.entries(file.variants ?? {})) {
    const value = variant?.payload?.value;
    if (variant?.enabled === true && typeof value === "string") variants[key] = value;
  }
  return { flags, variants };
}

/** How a host fetches the flags file: a GET of `url`, its status and body as text. */
export type FlagsTransport = (url: string, init: { signal?: AbortSignal }) => Promise<{ status: number; body: string }>;

/**
 * One app's flags file at `service` (the host's, normalized), read once through `transport`: the Shop's `dapps`
 * unless another app is named (the Explorer's `explorer`).
 */
export async function readFlagsFile(service: string, transport: FlagsTransport, signal?: AbortSignal, app: string = FLAGS_APP): Promise<RemoteFlagsAnswer> {
  const res = await transport(`${service}/${app}.json`, { signal });
  if (res.status < 200 || res.status >= 300) throw new Error(`feature flags: HTTP ${res.status}`);
  return parseRemoteFlags(JSON.parse(res.body));
}

/** One rollout flag in an answer: false without one, or unless it says yes. */
export function remoteFlagOn(answer: Pick<RemoteFlagsAnswer, "flags"> | null | undefined, flag: RemoteFlag): boolean {
  return answer?.flags[`${FLAGS_APP}-${flag}`] === true;
}

/** A rollout flag's variant payload in an answer (upstream's getAddressListVariant reads it), or null without one. */
export function remoteFlagVariantIn(answer: Pick<RemoteFlagsAnswer, "variants"> | null | undefined, flag: RemoteFlag): string | null {
  return answer?.variants[`${FLAGS_APP}-${flag}`] ?? null;
}

export type ShopFlags = {
  creatorSales: boolean;
};

/** The switches a host may declare itself (the native shop, a journey): a declared one stands over the file. */
export type HostShopFlags = Partial<ShopFlags> & { marketplaceFeatures?: boolean; myStore?: boolean };

/** A switch as the host declared it, else as the file answered (`null` while it has not). */
export function declaredOr(declared: HostShopFlags, key: keyof HostShopFlags, remote: boolean | null): boolean | null {
  const value = declared[key];
  return typeof value === "boolean" ? value : remote;
}

/** The Shop's switches: each as the host declared it, else as the file answered; off until one says yes. */
export function resolveShopFlags(declared: HostShopFlags, remote: (flag: RemoteFlag) => boolean): ShopFlags {
  return { creatorSales: declaredOr(declared, "creatorSales", remote(REMOTE_FLAG.creatorSales)) === true };
}
