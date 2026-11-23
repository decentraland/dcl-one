import { collectCampaignParams, type CampaignParams } from "@/core/lib/telemetry/campaign-params";
import { track } from "@/core/lib/telemetry/track";
import { trackedUserDropped } from "@/core/lib/telemetry/tracked-user";

import { downloadCtaClicked, readDownloadClickCorrelation, type DownloadClickSession } from "./download-click";

// The legacy site's Explorer download funnel (decentraland/sites `downloadTracking.ts`, `downloadFunnelExit.ts`,
// `downloadPageExit.ts`, `pages/DownloadSuccess`). Warehouse models read these rows by name and property, so every
// name, value, key order and omission below is the legacy one. Each event went out over the unload-safe beacon on
// exempt sessions and for bots too, which here is a conversion.

export type FunnelOs = "Windows" | "macOS";
export type FunnelArch = "amd64" | "arm64";
type AuthState = "authenticated" | "anonymous";
type MacArch = "apple_silicon" | "intel" | "unknown";

// Every legacy `DownloadPlace` passes `?place=`, the ones no legacy page linked from included (external landings).
const PLACES = [
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
export type FunnelPlace = (typeof PLACES)[number] | "unknown";

export function resolveDownloadPlace(value: string | null | undefined): FunnelPlace {
  return (PLACES as readonly string[]).includes(value ?? "") ? (value as FunnelPlace) : "unknown";
}

const MAC_ARCH: readonly string[] = ["apple_silicon", "intel", "unknown"];
export function resolveMacArch(value: string | null | undefined): MacArch | undefined {
  return MAC_ARCH.includes(value ?? "") ? (value as MacArch) : undefined;
}

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
/** The legacy `anon_user_id`: the URL's, else the visitor's anonymous id; only a UUID either way. */
export function resolveAnonUserId(fromUrl: string | null | undefined, sid: string): string | undefined {
  if (fromUrl && UUID_RE.test(fromUrl)) return fromUrl;
  return UUID_RE.test(sid) ? sid : undefined;
}

type Audit = { track_called_at: number; track_delivered_at: number; track_deferred: true };
type Correlated = { click_id?: string; ms_since_click?: number };

/** Matches the launcher's first-run fingerprint emitter field for field. */
type Fingerprint = {
  fp_screen_width: number;
  fp_screen_height: number;
  fp_device_pixel_ratio: number;
  fp_hardware_concurrency: number | null;
  fp_timezone: string | null;
  fp_language: string | null;
  fp_platform: string | null;
};

export type FunnelExtra = Partial<Fingerprint> & CampaignParams & Correlated & { mac_arch?: MacArch; download_target: "desktop_installer" };

type Base = FunnelExtra & {
  href: string;
  os: FunnelOs;
  arch: FunnelArch;
  auth_state: AuthState;
  revisit: number;
  place?: Exclude<FunnelPlace, "unknown">;
  anon_user_id?: string;
};

type DeliveryMode = "streamed" | "anchor_fallback" | "anchor_native";

/** One attempt's context. `place` is left out of its events when unknown, `anon_user_id` when absent. */
export type DownloadAttempt = {
  href: string;
  os: FunnelOs;
  arch: FunnelArch;
  place: FunnelPlace;
  anonUserId?: string;
  authenticated: boolean;
  revisit: number;
  extra: FunnelExtra;
};

function audit(): Audit {
  const now = Date.now();
  // The beacon never learns when it was delivered, so the legacy fields mirror the call time.
  return { track_called_at: now, track_delivered_at: now, track_deferred: true };
}

function sendContext({ sid, userId }: DownloadClickSession) {
  // A sign-out in progress leaves the account out before root's data has reloaded without it.
  const user = userId && !trackedUserDropped() ? userId : undefined;
  return { sid, ...(user ? { userId: user } : {}), legacy: true, conversion: true } as const;
}

const authState = (authenticated: boolean): AuthState => (authenticated ? "authenticated" : "anonymous");

/** Never throws: a fingerprint the browser hides leaves only the `fp_*` fields out. */
function fingerprint(): Partial<Fingerprint> {
  try {
    let timezone: string | null = null;
    try {
      timezone = Intl.DateTimeFormat().resolvedOptions().timeZone || null;
    } catch {
      // Locked-down environments.
    }
    return {
      fp_screen_width: window.screen?.width ?? 0,
      fp_screen_height: window.screen?.height ?? 0,
      fp_device_pixel_ratio: window.devicePixelRatio ?? 1,
      fp_hardware_concurrency: navigator.hardwareConcurrency ?? null,
      fp_timezone: timezone,
      fp_language: navigator.language ?? null,
      fp_platform: navigator.platform ?? null,
    };
  } catch {
    return {};
  }
}

function correlated(now = Date.now()): Correlated {
  const correlation = readDownloadClickCorrelation();
  return correlation ? { click_id: correlation.click_id, ms_since_click: now - correlation.clicked_at } : {};
}

/** The `extra` every /download_success attempt carries, read at the moment the attempt starts. */
export function funnelExtra(macArch: MacArch | undefined): FunnelExtra {
  return {
    ...fingerprint(),
    ...collectCampaignParams(),
    ...correlated(),
    ...(macArch ? { mac_arch: macArch } : {}),
    download_target: "desktop_installer",
  };
}

/** `downloadSuccess:visits:<os>:<arch>`: 0 on the first visit of this tab, then one more per arrival. */
export function nextRevisit(os: FunnelOs, arch: FunnelArch): number {
  const key = `downloadSuccess:visits:${os}:${arch}`;
  try {
    const current = Number(sessionStorage.getItem(key) ?? "0");
    sessionStorage.setItem(key, String(current + 1));
    return current;
  } catch {
    return 0;
  }
}

function base(attempt: DownloadAttempt): Base {
  return {
    ...attempt.extra,
    href: attempt.href,
    os: attempt.os,
    arch: attempt.arch,
    auth_state: authState(attempt.authenticated),
    revisit: attempt.revisit,
    ...(attempt.place !== "unknown" ? { place: attempt.place } : {}),
    ...(attempt.anonUserId ? { anon_user_id: attempt.anonUserId } : {}),
  };
}

export type DownloadTracker = {
  started(): void;
  success(filename: string, delivery: { mode: DeliveryMode; bytes?: number; requestId?: string }): void;
  failed(reason: string): void;
};

/** One attempt's `download_started`, `download_success` and `download_failed`, timed from `started()`. */
export function createDownloadTracker(attempt: DownloadAttempt, session: DownloadClickSession): DownloadTracker {
  let startedAt: number | null = null;
  return {
    started() {
      startedAt = Date.now();
      track("download_started", { ...base(attempt), started_at: startedAt, ...audit() }, sendContext(session));
    },
    success(filename, { mode, bytes, requestId }) {
      const succeededAt = Date.now();
      const anchor = startedAt ?? succeededAt;
      track(
        "download_success",
        {
          delivery_mode: mode,
          ...(requestId ? { gateway_request_id: requestId } : {}),
          ...base(attempt),
          filename,
          started_at: anchor,
          succeeded_at: succeededAt,
          duration_ms: succeededAt - anchor,
          ...(bytes !== undefined ? { bytes_transferred: bytes } : {}),
          ...audit(),
        },
        sendContext(session),
      );
    },
    failed(reason) {
      const failedAt = Date.now();
      const anchor = startedAt ?? failedAt;
      track(
        "download_failed",
        { ...base(attempt), reason, started_at: anchor, failed_at: failedAt, duration_ms: failedAt - anchor, ...audit() },
        sendContext(session),
      );
    },
  };
}

type Arrival = { os: FunnelOs; arch: FunnelArch; place: FunnelPlace; revisit: number; authenticated: boolean };

/** Sent on arrival at /download_success, before anything else; `place` is kept even when unknown. */
export function sendDownloadSuccessArrived(arrival: Arrival, session: DownloadClickSession): void {
  const now = Date.now();
  track(
    "download_success_arrived",
    {
      os: arrival.os,
      arch: arrival.arch,
      place: arrival.place,
      revisit: arrival.revisit,
      auth_state: authState(arrival.authenticated),
      ...collectCampaignParams(),
      ...correlated(now),
      download_target: "desktop_installer",
      track_called_at: now,
      track_delivered_at: now,
      track_deferred: true,
    },
    sendContext(session),
  );
}

export type FunnelExit = {
  os: FunnelOs;
  arch: FunnelArch;
  place: FunnelPlace;
  startedFired: boolean;
  successFired: boolean;
  failedFired: boolean;
  msOnPage: number;
  revisit: number;
  authenticated: boolean;
  anonUserId?: string;
};

/** The /download_success snapshot on each hide: no audit fields, no `utm_*`. */
export function sendDownloadFunnelExit(exit: FunnelExit, session: DownloadClickSession): void {
  const clickId = readDownloadClickCorrelation()?.click_id;
  track(
    "download_funnel_exit",
    {
      os: exit.os,
      arch: exit.arch,
      place: exit.place,
      download_started_fired: exit.startedFired,
      download_success_fired: exit.successFired,
      download_failed_fired: exit.failedFired,
      ms_on_page: exit.msOnPage,
      revisit: exit.revisit,
      auth_state: authState(exit.authenticated),
      ...(exit.anonUserId ? { anon_user_id: exit.anonUserId } : {}),
      ...(clickId ? { click_id: clickId } : {}),
    },
    sendContext(session),
  );
}

/** The /download snapshot on each hide: whether a download control was clicked since the page mounted. */
export function sendDownloadPageExit(msOnPage: number, session: DownloadClickSession): void {
  track("download_page_exit", { cta_clicked: downloadCtaClicked(), ms_on_page: msOnPage, ...collectCampaignParams() }, sendContext(session));
}

/** Calls `onHidden` on every `visibilitychange` to hidden; the legacy events have no fire-once guard. */
export function onPageHidden(onHidden: () => void): () => void {
  const listener = () => {
    if (document.hidden) onHidden();
  };
  document.addEventListener("visibilitychange", listener);
  return () => document.removeEventListener("visibilitychange", listener);
}
