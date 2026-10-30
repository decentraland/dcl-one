// The ONE base URL contract. CATALYST_URL names the v2 node the SSR process
// reads; every other service base derives from it as a path prefix the node
// mounts (src/main.rs). The old per-service env names stay as overrides.

export const DEFAULT_CATALYST_URL = "http://127.0.0.1:5210";

export const WORLDS_PREFIX = "/worlds-content-server";
export const SOCIAL_PREFIX = "/social-api";
export const NOTIFICATIONS_PREFIX = "/notifications";
export const TELEMETRY_PREFIX = "/telemetry";
export const MARKET_PREFIX = "/marketplace-api";
export const BUILDER_PREFIX = "/builder-api";
export const DEFAULT_CREATORS_DATA_URL = "/api/creator-hub";
export const DEFAULT_PLAY_PATH = "/play";

export type BaseEnv = Record<string, string | undefined>;

export function processEnv(): BaseEnv {
  return typeof process !== "undefined" && process.env ? process.env : {};
}

function trimSlash(value: string): string {
  return value.replace(/\/$/, "");
}

function envValue(env: BaseEnv, name: string): string | undefined {
  const v = env[name];
  return v && v.trim() !== "" ? v.trim() : undefined;
}

function browserBase(): string | undefined {
  if (typeof window === "undefined") return undefined;
  const w = window as Window & { __CATALYST_BASE__?: string };
  return w.__CATALYST_BASE__ ?? window.location?.origin ?? undefined;
}

/** The node the SSR process reads (server) or the page origin (browser). */
export function catalystUrl(override?: string, env: BaseEnv = processEnv()): string {
  if (override) return trimSlash(override);
  if (typeof window === "undefined") {
    return trimSlash(envValue(env, "CATALYST_URL") ?? DEFAULT_CATALYST_URL);
  }
  return trimSlash(browserBase() ?? envValue(env, "CATALYST_URL") ?? DEFAULT_CATALYST_URL);
}

/** The same node as browsers reach it: links, thumbnails, window.__CATALYST_BASE__. */
export function publicCatalystUrl(env: BaseEnv = processEnv()): string {
  return trimSlash(envValue(env, "PUBLIC_CATALYST_URL") ?? catalystUrl(undefined, env));
}

/** Set only when PUBLIC_CATALYST_URL differs from the page origin. */
export function browserCatalystBase(env: BaseEnv = processEnv()): string {
  return trimSlash(envValue(env, "PUBLIC_CATALYST_URL") ?? "");
}

export function worldsUrl(override?: string, env: BaseEnv = processEnv()): string {
  return trimSlash(
    override ?? envValue(env, "WORLDS_URL") ?? `${catalystUrl(undefined, env)}${WORLDS_PREFIX}`,
  );
}

export function publicWorldsUrl(env: BaseEnv = processEnv()): string {
  return `${publicCatalystUrl(env)}${WORLDS_PREFIX}`;
}

export function worldsContentServerUrl(override?: string, env: BaseEnv = processEnv()): string {
  return trimSlash(
    override ?? envValue(env, "WORLDS_CONTENT_SERVER_URL") ?? worldsUrl(undefined, env),
  );
}

export function communitiesUrl(env: BaseEnv = processEnv()): string {
  return trimSlash(
    envValue(env, "COMMUNITIES_URL") ?? `${catalystUrl(undefined, env)}${SOCIAL_PREFIX}`,
  );
}

/** Unset means the sink and the flag probe are no-ops (README contract). */
export function telemetryUrl(env: BaseEnv = processEnv()): string | undefined {
  const v = envValue(env, "TELEMETRY_URL");
  return v ? trimSlash(v) : undefined;
}

export function creatorsDataUrl(override?: string, env: BaseEnv = processEnv()): string {
  return trimSlash(override ?? envValue(env, "CREATORS_DATA_URL") ?? DEFAULT_CREATORS_DATA_URL);
}

/**
 * A path of this site as a link: under PUBLIC_CATALYST_URL (server) or
 * window.__CATALYST_BASE__ (browser) when the public origin is configured,
 * else relative to the page origin. Both sides agree, so SSR and hydration
 * render the same href.
 */
export function siteUrl(path: string, env: BaseEnv = processEnv()): string {
  const p = path.startsWith("/") ? path : `/${path}`;
  if (typeof window === "undefined") return `${browserCatalystBase(env)}${p}`;
  const w = window as Window & { __CATALYST_BASE__?: string };
  return `${trimSlash(w.__CATALYST_BASE__ ?? "")}${p}`;
}

/**
 * The explorer entry every jump-in link targets: BEVY_PLAY_URL (default /play;
 * a path on this origin, or an absolute URL) with the launch parameters.
 * `playUrl({ position: "-3,-2" })` -> `/play/?position=-3%2C-2`.
 */
export function playUrl(
  params?: Record<string, string | undefined> | URLSearchParams,
  env: BaseEnv = processEnv(),
): string {
  const configured = envValue(env, "BEVY_PLAY_URL") ?? DEFAULT_PLAY_PATH;
  const base = /^https?:\/\//.test(configured)
    ? `${trimSlash(configured)}/`
    : `${siteUrl(trimSlash(configured), env)}/`;
  const search =
    params instanceof URLSearchParams
      ? params
      : new URLSearchParams(
          Object.entries(params ?? {}).filter((kv): kv is [string, string] => kv[1] !== undefined),
        );
  const qs = search.toString();
  return qs ? `${base}?${qs}` : base;
}
