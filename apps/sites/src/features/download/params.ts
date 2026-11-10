// Query-parameter rules of the source Sites download flow (decentraland/sites@cfdfa1a), contract
// §2-§4 in the download contract. Every value read from a URL is validated before it is forwarded
// to the gateway, the launcher or telemetry.

export type DownloadOs = "windows" | "macos";
export type DownloadArch = "amd64" | "arm64";
export type MacArch = "apple_silicon" | "intel" | "unknown";

const ARCHS = new Set<string>(["amd64", "arm64"]);
const MAC_ARCHS = new Set<string>(["apple_silicon", "intel", "unknown"]);

export const CAMPAIGN_PARAM_KEYS = [
  "utm_org",
  "utm_source",
  "utm_medium",
  "utm_campaign",
  "utm_content",
  "utm_term",
] as const;
const MAX_CAMPAIGN_VALUE_LENGTH = 256;

export const DOWNLOAD_PLACES = [
  "landing-hero",
  "landing-hero-epic",
  "landing-hero-platform-switch",
  "come-hang-out",
  "come-hang-out-platform-switch",
  "jump-in-already-user",
  "download-page",
  "download-modal",
  "download-success-footer",
  "creator-hub-download-page",
  "creator-hub-success-page",
  "creators-hero",
  "jump-in-landing",
  "lets-play-landing",
] as const;
export type DownloadPlace = (typeof DOWNLOAD_PLACES)[number] | "unknown";

const UUID_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const REFERRER_RE = /^0x[a-fA-F0-9]{40}$/;
const REALM_RE = /^[a-zA-Z0-9._-]{1,64}$/;
const COMMUNITY_RE = /^[a-zA-Z0-9-]{1,64}$/;
const DEFAULT_POSITION = "0,0";
const DEFAULT_REALM = "main";

/** The os label source Sites puts on URLs and telemetry. */
export function osLabel(os: DownloadOs): "Windows" | "macOS" {
  return os === "windows" ? "Windows" : "macOS";
}

/** os: windows|macos, anything else macOS. arch: amd64|arm64, default amd64 on Windows, arm64 on macOS. */
export function resolvePlatform(params: URLSearchParams): {
  os: DownloadOs;
  arch: DownloadArch;
} {
  const os: DownloadOs =
    (params.get("os") ?? "").toLowerCase() === "windows" ? "windows" : "macos";
  const fallback: DownloadArch = os === "windows" ? "amd64" : "arm64";
  const raw = params.get("arch") ?? fallback;
  return { os, arch: ARCHS.has(raw) ? (raw as DownloadArch) : fallback };
}

/** Analytics only: never selects a binary. */
export function resolveMacArch(params: URLSearchParams): MacArch | undefined {
  const raw = params.get("mac_arch") ?? "";
  return MAC_ARCHS.has(raw) ? (raw as MacArch) : undefined;
}

export function resolvePlace(value: string | null): DownloadPlace {
  return (DOWNLOAD_PLACES as readonly string[]).includes(value ?? "")
    ? (value as DownloadPlace)
    : "unknown";
}

function normalizeCampaignValue(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/\s+/g, "_")
    .replace(/[^a-z0-9_-]/g, "")
    .replace(/_+/g, "_")
    .replace(/^_+|_+$/g, "")
    .slice(0, MAX_CAMPAIGN_VALUE_LENGTH);
}

export function collectCampaignParams(
  params: URLSearchParams,
): Record<string, string> {
  const collected: Record<string, string> = {};
  for (const key of CAMPAIGN_PARAM_KEYS) {
    const value = params.get(key);
    const normalized = value ? normalizeCampaignValue(value) : "";
    if (normalized) collected[key] = normalized;
  }
  return collected;
}

export type DeepLinkParams = {
  position?: string;
  realm?: string;
  communityId?: string;
};

/** position is re-emitted as integer "x,y"; realm and communityId must match their character sets. */
export function collectDeepLinkParams(params: URLSearchParams): DeepLinkParams {
  const out: DeepLinkParams = {};
  const tokens = (params.get("position") ?? "").split(/[,.]/);
  if (tokens.length === 2) {
    const [x, y] = tokens.map((token) => Number.parseInt(token, 10));
    if (Number.isFinite(x) && Number.isFinite(y)) {
      const position = `${x},${y}`;
      if (position !== DEFAULT_POSITION) out.position = position;
    }
  }
  const realm = params.get("realm");
  if (realm && REALM_RE.test(realm) && realm !== DEFAULT_REALM)
    out.realm = realm;
  const communityId = params.get("communityId");
  if (communityId && COMMUNITY_RE.test(communityId))
    out.communityId = communityId;
  return out;
}

export function parseReferrer(
  value: string | null | undefined,
): string | undefined {
  return value && REFERRER_RE.test(value) ? value.toLowerCase() : undefined;
}

export function parseAnonUserId(
  value: string | null | undefined,
): string | undefined {
  return value && UUID_RE.test(value) ? value : undefined;
}

export const ANON_USER_ID_PARAM = "anon_user_id";

/** The /download_success URL a download CTA navigates to. Routing params are never overwritten. */
export function downloadSuccessHref(
  os: DownloadOs,
  place: DownloadPlace,
  options: {
    arch?: DownloadArch;
    anonUserId?: string;
    referrer?: string;
    deepLink?: DeepLinkParams;
    campaign?: Record<string, string>;
  } = {},
): string {
  const params = new URLSearchParams({ os: osLabel(os), place });
  if (options.anonUserId) params.set(ANON_USER_ID_PARAM, options.anonUserId);
  if (options.arch) params.set("arch", options.arch);
  if (options.referrer) params.set("referrer", options.referrer);
  for (const [key, value] of Object.entries({
    ...options.deepLink,
    ...options.campaign,
  }))
    if (value && !params.has(key)) params.set(key, value);
  return `/download_success?${params.toString()}`;
}
