import type { FunnelArch, FunnelOs } from "./download-funnel";

// How /download_success picks the installer URL, after the legacy site (decentraland/sites `downloadWithIdentity.ts`,
// `url.ts` `calculateCDNReleaseLinksWithIdentity`, decentraland-ui2 `cdnReleases.ts`). The download gateway wraps the
// installer with the visitor's sign-in token (identity route) or campaign id (anonymous route); without either, or
// without a gateway, the plain installer from the installer host is served.

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const REFERRER_RE = /^0x[a-fA-F0-9]{40}$/;
const REALM_RE = /^[a-zA-Z0-9._-]{1,64}$/;
const REFERRER_STORAGE_KEY = "dcl_referrer";
const ATTRIBUTION_STORAGE_KEY = "dcl_gateway_anon_user_id";
const IDENTITY_TIMEOUT_MS = 5000;

export type DeepLinkParams = { position?: string; realm?: string };

/** `position` re-emitted as `x,y` from its integers and `realm` allowlisted; the Explorer's defaults are dropped. */
export function collectDeepLinkParams(params: URLSearchParams): DeepLinkParams {
  const out: DeepLinkParams = {};
  const tokens = params.get("position")?.split(/[,.]/);
  if (tokens?.length === 2) {
    const [x, y] = tokens.map((token) => Number.parseInt(token, 10));
    const position = Number.isFinite(x) && Number.isFinite(y) ? `${x},${y}` : undefined;
    if (position && position !== "0,0") out.position = position;
  }
  const realm = params.get("realm");
  if (realm && REALM_RE.test(realm) && realm !== "main") out.realm = realm;
  return out;
}

/**
 * The referral wallet the installer carries: the URL's `?referrer=` when present (an invalid one clears the stored
 * value and sends none), else the one the invite page stored for this tab. Lowercase 0x address or null.
 */
export function resolveReferrer(params: URLSearchParams): string | null {
  const parse = (value: string | null | undefined) => (value && REFERRER_RE.test(value) ? value.toLowerCase() : null);
  try {
    const raw = params.get("referrer");
    if (raw !== null) {
      const referrer = parse(raw);
      if (!referrer) sessionStorage.removeItem(REFERRER_STORAGE_KEY);
      return referrer;
    }
    return parse(sessionStorage.getItem(REFERRER_STORAGE_KEY));
  } catch {
    return parse(params.get("referrer"));
  }
}

let memory: { visitorId: string; attributionId: string } | null = null;

/**
 * The `anon_user_id` the gateway gets: the visitor's UUID; for a visitor whose id is not a UUID, a separate UUID kept
 * per visitor id, but only when the installer must carry a deep link or a referrer (the anonymous route needs one).
 */
export function resolveGatewayAnonUserId(
  anonUserId: string | undefined,
  visitorId: string,
  deepLink: DeepLinkParams,
  referrer: string | null,
): string | undefined {
  if (anonUserId && UUID_RE.test(anonUserId)) return anonUserId;
  if (!deepLink.position && !deepLink.realm && !referrer) return undefined;
  if (UUID_RE.test(visitorId)) return visitorId;
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(ATTRIBUTION_STORAGE_KEY) || "null");
    const { segmentId, attributionId } = (stored ?? {}) as Record<string, unknown>;
    if (segmentId === visitorId && typeof attributionId === "string" && UUID_RE.test(attributionId)) {
      memory = { visitorId, attributionId };
      return attributionId;
    }
  } catch {
    // Corrupt or blocked storage must not break the download.
  }
  if (memory?.visitorId !== visitorId) memory = { visitorId, attributionId: crypto.randomUUID() };
  try {
    localStorage.setItem(ATTRIBUTION_STORAGE_KEY, JSON.stringify({ segmentId: visitorId, attributionId: memory.attributionId }));
  } catch {
    // Memory keeps retries stable.
  }
  return memory.attributionId;
}

export type InstallerChoice = {
  os: FunnelOs;
  arch: FunnelArch;
  /** The plain installer from the installer host for this system and architecture. */
  fallback: string;
  /** The download gateway's base, or null when the deployment configures none. */
  gateway: string | null;
  /** The sign-in token the gateway wraps in, for a signed-in visitor; resolves undefined otherwise. */
  identityId: (signal: AbortSignal) => Promise<string | undefined>;
  anonUserId: string | undefined;
  signal: AbortSignal;
};

function gatewayUrl(gateway: string, segment: string, os: FunnelOs, arch: FunnelArch, fallback: string): string {
  if (os === "Windows") return `${gateway}/${segment}/decentraland.exe`;
  // The identity route has no Intel Mac build: that visitor keeps the plain installer, as the legacy table did.
  if (segment !== "anonymous" && arch === "amd64") return fallback;
  return `${gateway}/${segment}/decentraland.dmg`;
}

/** The installer URL before its query: identity route, then anonymous route, then the plain installer. */
export async function chooseInstallerUrl({ os, arch, fallback, gateway, identityId, anonUserId, signal }: InstallerChoice): Promise<string> {
  if (!gateway) return fallback;
  let id: string | undefined;
  const timeout = new AbortController();
  const timer = setTimeout(() => timeout.abort(new DOMException("The sign-in token took too long", "TimeoutError")), IDENTITY_TIMEOUT_MS);
  try {
    id = await identityId(AbortSignal.any([signal, timeout.signal]));
  } catch {
    // A failed or slow sign-in token falls back to the anonymous route, as the legacy page did.
  } finally {
    clearTimeout(timer);
  }
  if (id) return gatewayUrl(gateway, encodeURIComponent(id), os, arch, fallback);
  if (anonUserId) return gatewayUrl(gateway, "anonymous", os, arch, fallback);
  return fallback;
}

/** Appends the attribution the gateway bakes into the installer. Never `click_id`: a UUID there shadows the token. */
export function withInstallerQuery(url: string, params: Record<string, string | undefined | null>): string {
  const target = new URL(url);
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== null) target.searchParams.append(key, value);
  }
  return target.toString();
}

/** The gateway's sign-in token for this tab's signed-in identity (`POST /identities`); undefined when signed out. */
export function signInTokenFrom(authApiUrl: string): (signal: AbortSignal) => Promise<string | undefined> {
  return async (signal) => {
    const [{ getIdentity }, { postIdentityHandoff }] = await Promise.all([
      import("@/data/lib/auth/session"),
      import("@/data/lib/auth/deeplink-identity"),
    ]);
    const identity = getIdentity();
    if (!identity) return undefined;
    return (await postIdentityHandoff(identity, authApiUrl, { signal })).identityId;
  };
}
