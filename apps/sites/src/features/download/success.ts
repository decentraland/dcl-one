import type { postIdentityHandoff } from "@/data/lib/auth/deeplink-identity";
import type { AuthIdentity } from "@/data/lib/auth/types";

import { deliverInstaller, type DeliveryResult } from "./delivery";
import {
  cdnInstallerUrl,
  resolveInstallerUrl,
  type InstallerUrl,
} from "./links";
import { parseAnonUserId, type DownloadArch, type DownloadOs } from "./params";

// The /download_success download run (download contract §3 and §6, plan behavior 4).

const VISITS_PREFIX = "downloadSuccess:visits:";
const GATEWAY_ANON_KEY = "dcl_gateway_anon_user_id";
export const IDENTITY_TIMEOUT_MS = 5000;
/** Same-origin auth-api, proxied by the catalyst (approved for the handoff). */
export const AUTH_API_URL = "/auth-api";

/** Bumps the per-tab visit count for this artifact; returns the previous count (0 = first visit). */
export function nextRevisit(os: DownloadOs, arch: DownloadArch): number {
  const key = `${VISITS_PREFIX}${os}:${arch}`;
  try {
    const previous =
      Number.parseInt(sessionStorage.getItem(key) ?? "0", 10) || 0;
    sessionStorage.setItem(key, String(previous + 1));
    return previous;
  } catch {
    return 0;
  }
}

/**
 * The id the gateway bakes into the installer: the one the link carried, else the telemetry sid when
 * it is a UUID, else a UUID kept in localStorage (as source does when the Segment id is not one).
 */
export function gatewayAnonUserId(
  fromUrl: string | null | undefined,
  sid: string,
): string {
  const carried = parseAnonUserId(fromUrl) ?? parseAnonUserId(sid);
  if (carried) return carried;
  try {
    const stored = parseAnonUserId(localStorage.getItem(GATEWAY_ANON_KEY));
    if (stored) return stored;
    const minted = crypto.randomUUID();
    localStorage.setItem(GATEWAY_ANON_KEY, minted);
    return minted;
  } catch {
    return crypto.randomUUID();
  }
}

/**
 * The signed-in identity id for the gateway URL. Every failure (auth error, timeout, no id) yields
 * undefined, so the caller downloads through the anonymous gateway instead (plan behavior 4a).
 */
export async function resolveIdentityId(
  identity: AuthIdentity | null,
  options: {
    signal: AbortSignal;
    post?: typeof postIdentityHandoff;
    timeoutMs?: number;
  },
): Promise<string | undefined> {
  if (!identity) return undefined;
  const signal = AbortSignal.any([
    options.signal,
    AbortSignal.timeout(options.timeoutMs ?? IDENTITY_TIMEOUT_MS),
  ]);
  try {
    // Loaded on demand: the handoff pulls in viem, which signed-out visitors never need.
    const post =
      options.post ??
      (await import("@/data/lib/auth/deeplink-identity")).postIdentityHandoff;
    return (await post(identity, AUTH_API_URL, { signal })).identityId;
  } catch {
    return undefined;
  }
}

/**
 * The installer URL as telemetry sees it. The identity id in the path is a sign-in handoff the
 * launcher authenticates with, so it is replaced; source Sites sends it as is.
 */
export function telemetryHref(installer: InstallerUrl): string {
  if (installer.source !== "identity") return installer.url;
  const url = new URL(installer.url);
  url.pathname = url.pathname.replace(
    /^(.*)\/[^/]+(\/[^/]+)$/,
    "$1/<identity>$2",
  );
  return url.toString();
}

export type DownloadTracker = {
  started(now?: number): void;
  success(
    result: {
      filename: string;
      deliveryMode: DeliveryResult["deliveryMode"];
      gatewayRequestId?: string;
      bytes?: number;
    },
    now?: number,
  ): void;
  failed(reason: string, now?: number): void;
};

export type DownloadOutcome =
  | { status: "success"; installer: InstallerUrl; delivery: DeliveryResult }
  | { status: "failed"; installer: InstallerUrl; reason: string }
  | { status: "aborted" };

/**
 * Resolves the installer URL (identity → anonymous gateway → CDN), then delivers it, reporting the
 * lifecycle events through a tracker built for the final URL. An abort reports nothing.
 */
export async function runDownload(input: {
  os: DownloadOs;
  arch: DownloadArch;
  gateway?: string;
  identity: AuthIdentity | null;
  anonUserId: string;
  query: Record<string, string | undefined>;
  signal: AbortSignal;
  tracker: (href: string) => DownloadTracker;
  onProgress?: (percent: number) => void;
  resolveIdentity?: typeof resolveIdentityId;
  deliver?: typeof deliverInstaller;
}): Promise<DownloadOutcome> {
  const { os, arch, signal } = input;
  let installer: InstallerUrl = {
    url: cdnInstallerUrl(os, arch),
    source: "cdn",
    filename: os === "windows" ? "decentraland.exe" : "decentraland.dmg",
  };
  let tracker: DownloadTracker | undefined;
  try {
    const identityId = input.gateway
      ? await (input.resolveIdentity ?? resolveIdentityId)(input.identity, {
          signal,
        })
      : undefined;
    if (signal.aborted) return { status: "aborted" };
    installer = resolveInstallerUrl({
      os,
      arch,
      gateway: input.gateway,
      identityId,
      anonUserId: input.anonUserId,
      query: input.query,
    });
    tracker = input.tracker(telemetryHref(installer));
    tracker.started();
    const delivery = await (input.deliver ?? deliverInstaller)({
      url: installer.url,
      filename: installer.filename,
      os,
      signal,
      onProgress: input.onProgress,
    });
    if (signal.aborted) return { status: "aborted" };
    tracker.success({
      filename: installer.filename,
      deliveryMode: delivery.deliveryMode,
      gatewayRequestId: delivery.gatewayRequestId,
      bytes: delivery.bytesTransferred,
    });
    return { status: "success", installer, delivery };
  } catch (error) {
    if (signal.aborted) return { status: "aborted" };
    const reason = error instanceof Error ? error.message : String(error);
    // Without a resolved URL, the failure is reported against the CDN link, as source does.
    (tracker ?? input.tracker(telemetryHref(installer))).failed(reason);
    return { status: "failed", installer, reason };
  }
}
