// Which upstream service a request belongs to, for both sides of a parity capture.
//
// The reference shop calls the production hosts directly; ui3 calls the node's canonical
// `/shop-api` (and the compatibility mounts), which apps/ui3/src/shop/api/endpoints.ts maps onto the
// same upstream routes. Both resolve to one (service, path) so a recorded answer is served to both.
import { ENDPOINTS, SHOP_API_PREFIX, type Endpoint } from "../../../apps/ui3/src/shop/api/endpoints.ts";

export type Service =
  | "marketplace"
  | "credits"
  | "shop-server"
  | "builder"
  | "peer"
  | "feature-flags"
  | "cms"
  | "places"
  | "camera-reel"
  | "rpc"
  | "notifications"
  | "transactions"
  | "signatures"
  | "fonts"
  | "fonts-static"
  | "wearable-preview"
  | "offers"
  | "node";

/** The production origin of each upstream service; record mode reads these, replay never does. */
export const ORIGINS: Partial<Record<Service, string>> = {
  marketplace: "https://marketplace-api.decentraland.org",
  credits: "https://credits.decentraland.org",
  "shop-server": "https://shop-api.decentraland.org",
  builder: "https://builder-api.decentraland.org",
  peer: "https://peer.decentraland.org",
  "feature-flags": "https://feature-flags.decentraland.org",
  cms: "https://cms-api.decentraland.org",
  places: "https://places.decentraland.org",
  "camera-reel": "https://camera-reel-service.decentraland.org",
  rpc: "https://rpc.decentraland.org",
  notifications: "https://notifications.decentraland.org",
  transactions: "https://transactions-api.decentraland.org",
  signatures: "https://signatures-api.decentraland.org",
  fonts: "https://fonts.googleapis.com",
  "fonts-static": "https://fonts.gstatic.com",
};

const HOSTS = new Map<string, Service>(Object.entries(ORIGINS).map(([s, o]) => [new URL(o).host, s as Service]));
HOSTS.set("wearable-preview.decentraland.org", "wearable-preview");

export function serviceForHost(host: string): Service | undefined {
  return HOSTS.get(host);
}

/** Third-party telemetry and payment scripts: answered empty, never recorded. */
const BLOCKED = [
  /(^|\.)segment\.(com|io)$/,
  /(^|\.)sentry\.io$/,
  /(^|\.)google-analytics\.com$/,
  /(^|\.)googletagmanager\.com$/,
  /(^|\.)doubleclick\.net$/,
  /(^|\.)hotjar\.(com|io)$/,
  /(^|\.)intercom(cdn)?\.(io|com)$/,
  /(^|\.)stripe\.(com|network)$/,
  /(^|\.)walletconnect\.(com|org)$/,
  /(^|\.)reown\.com$/,
  /(^|\.)thirdweb\.com$/,
  /(^|\.)growthbook\.io$/,
];

export type Resolved =
  | { kind: "api"; service: Service; path: string }
  | { kind: "image" }
  | { kind: "blocked" }
  | { kind: "unknown"; host: string };

/** The node mounts whose pictures are content (thumbnails, avatars, outfit renders), never app assets. */
export const OWN_IMAGE_MOUNTS = /^\/(lambdas|content|media|profile-images|shop-api\/v1\/outfits\/thumbnails)\//;

const IMAGE_EXT = /\.(png|jpe?g|webp|gif|avif|svg|ico)$/i;

/** Requests that are pictures of content (thumbnails, avatars, covers): answered with a placeholder. */
export function isContentImage(url: URL, resourceType?: string): boolean {
  if (resourceType === "image") return true;
  const p = url.pathname;
  if (IMAGE_EXT.test(p)) return true;
  if (/\/(thumbnail|image|face256|face|body)$/.test(p)) return true;
  if (/\/v1\/outfits\/thumbnails\/[^/]+$/.test(p)) return true;
  if (/^\/(lambdas\/collections\/contents|content\/contents|media|profile-images)\//.test(p) && !/\/entities?\//.test(p)) return true;
  return false;
}

function matchTemplate(template: string, path: string): boolean {
  const re = new RegExp(`^${template.replace(/[.*+?^$()|[\]\\]/g, "\\$&").replace(/\\?\{[A-Za-z]+\\?\}/g, "[^/]+")}$`);
  return re.test(path);
}

function endpointFor(method: string, path: string): Endpoint | null {
  let best: Endpoint | null = null;
  for (const e of Object.values(ENDPOINTS) as Endpoint[]) {
    if (e.method !== method || !matchTemplate(e.path, path)) continue;
    // A literal segment beats a template one (/v1/trades-mirror over /v1/trades/{id}).
    if (!best || e.path.split("{").length < best.path.split("{").length) best = e;
  }
  return best;
}

/** A canonical `/shop-api/<path>` request as the upstream route it stands for. */
export function fromShopApi(method: string, path: string): { service: Service; path: string } | null {
  const e = endpointFor(method, path);
  if (!e) return null;
  if (e.path === ENDPOINTS.purchaseOffers.path) return { service: "offers", path };
  const fill = (legacy: string) => {
    const names = [...e.path.matchAll(/\{([A-Za-z]+)\}/g)].map((m) => m[1]!);
    const values = path.match(new RegExp(`^${e.path.replace(/\{[A-Za-z]+\}/g, "([^/]+)")}$`))?.slice(1) ?? [];
    let out = legacy;
    names.forEach((n, i) => (out = out.replace(`{${n}}`, values[i] ?? "")));
    return out;
  };
  if (e.legacy === null) return { service: "shop-server", path };
  const legacy = fill(e.legacy);
  for (const [prefix, service] of [
    ["/marketplace-api", "marketplace"],
    ["/credits", "credits"],
    ["/transactions-api", "transactions"],
    ["/signatures-api", "signatures"],
  ] as const) {
    if (legacy.startsWith(`${prefix}/`)) return { service, path: legacy.slice(prefix.length) };
  }
  return null;
}

/** A request the dcl-one side makes to its own origin (or that sites makes to the node). */
export function fromNodePath(method: string, path: string): { service: Service; path: string } {
  if (path.startsWith(`${SHOP_API_PREFIX}/`)) {
    const r = fromShopApi(method, path.slice(SHOP_API_PREFIX.length));
    if (r) return r;
  }
  for (const [prefix, service] of [
    ["/marketplace-api", "marketplace"],
    ["/credits", "credits"],
    ["/transactions-api", "transactions"],
    ["/signatures-api", "signatures"],
    ["/builder-api", "builder"],
  ] as const) {
    if (path.startsWith(`${prefix}/`)) return { service, path: path.slice(prefix.length) };
  }
  if (path.startsWith("/rpc/")) return { service: "rpc", path: path.slice("/rpc".length) };
  if (path.startsWith("/lambdas/") || path.startsWith("/content/")) return { service: "peer", path };
  return { service: "node", path };
}

/** Classify a browser request of either side; `origin` is that side's own server. */
export function resolve(url: URL, method: string, origin: string, resourceType?: string): Resolved | { kind: "own" } {
  if (url.origin === origin) {
    if (isContentImage(url, resourceType) && OWN_IMAGE_MOUNTS.test(url.pathname)) return { kind: "image" };
    return { kind: "own" };
  }
  if (url.protocol === "data:" || url.protocol === "blob:") return { kind: "own" };
  if (BLOCKED.some((re) => re.test(url.hostname))) return { kind: "blocked" };
  const service = HOSTS.get(url.host);
  if (service === "fonts" || service === "fonts-static") return { kind: "api", service, path: url.pathname };
  if (isContentImage(url, resourceType)) return { kind: "image" };
  if (service) return { kind: "api", service, path: url.pathname };
  return { kind: "unknown", host: url.host };
}

/** The canonical query: parsed and re-serialized in key/value order, so encodings agree. */
export function canonicalQuery(search: string): string {
  const entries = [...new URLSearchParams(search)];
  entries.sort(([a, av], [b, bv]) => (a < b ? -1 : a > b ? 1 : av < bv ? -1 : av > bv ? 1 : 0));
  return new URLSearchParams(entries).toString();
}
