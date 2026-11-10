import {
  track as defaultTrack,
  type TrackContext,
} from "@/core/lib/telemetry/track";

import type { DeliveryMode } from "./delivery";
import {
  osLabel,
  type DownloadArch,
  type DownloadOs,
  type DownloadPlace,
  type MacArch,
} from "./params";

// The source Sites download funnel (decentraland/sites@cfdfa1a, download contract §1): the same
// event names and payload fields, sent through dcl-one's existing track() destination. Where the
// events land in the warehouse is decision D5, outside this module.

type Campaign = {
  utm_org?: string;
  utm_source?: string;
  utm_medium?: string;
  utm_campaign?: string;
  utm_content?: string;
  utm_term?: string;
};

type Audit = {
  track_called_at: number;
  track_delivered_at: number;
  track_deferred: true;
};
type Correlation = { click_id?: string; ms_since_click?: number };
type AuthState = "authenticated" | "anonymous";
type DownloadTarget =
  "desktop_installer" | "app_store" | "google_play" | "epic";

type LifecycleBase = Campaign &
  Correlation &
  Audit & {
    href: string;
    os: "Windows" | "macOS";
    arch: DownloadArch;
    auth_state: AuthState;
    revisit: number;
    place?: DownloadPlace;
    anon_user_id?: string;
    mac_arch?: MacArch;
    download_target: "desktop_installer";
  };

export type DownloadStartedProps = LifecycleBase & { started_at: number };
export type DownloadSuccessProps = LifecycleBase & {
  filename: string;
  started_at: number;
  succeeded_at: number;
  duration_ms: number;
  delivery_mode: DeliveryMode;
  gateway_request_id?: string;
  bytes_transferred?: number;
};
export type DownloadFailedProps = LifecycleBase & {
  reason: string;
  started_at: number;
  failed_at: number;
  duration_ms: number;
};
export type DownloadArrivedProps = Campaign &
  Correlation &
  Audit & {
    os: "Windows" | "macOS";
    arch: DownloadArch;
    place: DownloadPlace;
    revisit: number;
    auth_state: AuthState;
    download_target: "desktop_installer";
  };
export type DownloadFunnelExitProps = {
  os: "Windows" | "macOS";
  arch: DownloadArch;
  place: DownloadPlace;
  download_started_fired: boolean;
  download_success_fired: boolean;
  download_failed_fired: boolean;
  ms_on_page: number;
  revisit: number;
  auth_state: AuthState;
  anon_user_id?: string;
  click_id?: string;
};
export type DownloadPageExitProps = Campaign & {
  cta_clicked: boolean;
  ms_on_page: number;
};
export type DownloadRedirectFailedProps = Campaign & {
  os: string;
  arch?: DownloadArch;
  place: "download-page";
  reason: string;
  download_target: "desktop_installer";
};
export type DownloadClickProps = Campaign &
  Audit & {
    place: string;
    event: "Download";
    os?: string;
    download_target: DownloadTarget;
    click_id?: string;
    clicked_at?: number;
    mac_arch?: MacArch;
  };

type Track = typeof defaultTrack;

// --- Click → download correlation (sessionStorage survives the navigation to /download_success).
const CLICK_KEY = "downloadFunnel:lastClick";
const MAX_CLICK_AGE_MS = 30 * 60 * 1000;

export function recordClick(now = Date.now()): {
  click_id: string;
  clicked_at: number;
} {
  const correlation = { click_id: crypto.randomUUID(), clicked_at: now };
  try {
    sessionStorage.setItem(CLICK_KEY, JSON.stringify(correlation));
  } catch {
    // Storage blocked: the click still carries it; only the cross-page join is lost.
  }
  return correlation;
}

export function readClick(
  now = Date.now(),
): { click_id: string; clicked_at: number } | null {
  try {
    const parsed: unknown = JSON.parse(
      sessionStorage.getItem(CLICK_KEY) ?? "null",
    );
    if (!parsed || typeof parsed !== "object") return null;
    const { click_id, clicked_at } = parsed as Record<string, unknown>;
    if (typeof click_id !== "string" || typeof clicked_at !== "number")
      return null;
    return now - clicked_at > MAX_CLICK_AGE_MS
      ? null
      : { click_id, clicked_at };
  } catch {
    return null;
  }
}

const MAX_REASON_LENGTH = 200;

/**
 * Failure reasons are error messages, which can quote the installer URL, and that URL carries the
 * signed-in identity id. URLs are dropped and the text is capped before it leaves the browser.
 */
export function sanitizeReason(reason: string): string {
  return reason
    .replace(/\b[a-z][a-z0-9+.-]*:\/\/\S+/gi, "<url>")
    .slice(0, MAX_REASON_LENGTH);
}

function audit(now: number): Audit {
  return {
    track_called_at: now,
    track_delivered_at: now,
    track_deferred: true,
  };
}

function correlation(now: number): Correlation {
  const click = readClick(now);
  return click
    ? { click_id: click.click_id, ms_since_click: now - click.clicked_at }
    : {};
}

export type AttemptContext = {
  href: string;
  os: DownloadOs;
  arch: DownloadArch;
  authState: AuthState;
  revisit: number;
  place: DownloadPlace;
  anonUserId?: string;
  macArch?: MacArch;
  campaign: Campaign;
};

/** One download attempt's started/success/failed events, timed from `started()`. */
export function createAttemptTracker(
  ctx: AttemptContext,
  session: TrackContext,
  track: Track = defaultTrack,
) {
  let startedAt: number | null = null;
  const base = (now: number): LifecycleBase => ({
    ...ctx.campaign,
    ...correlation(now),
    ...(ctx.macArch ? { mac_arch: ctx.macArch } : {}),
    download_target: "desktop_installer",
    href: ctx.href,
    os: osLabel(ctx.os),
    arch: ctx.arch,
    auth_state: ctx.authState,
    revisit: ctx.revisit,
    ...(ctx.place !== "unknown" ? { place: ctx.place } : {}),
    ...(ctx.anonUserId ? { anon_user_id: ctx.anonUserId } : {}),
    ...audit(now),
  });
  return {
    started(now = Date.now()) {
      startedAt = now;
      track("download_started", { ...base(now), started_at: now }, session);
    },
    success(
      result: {
        filename: string;
        deliveryMode: DeliveryMode;
        gatewayRequestId?: string;
        bytes?: number;
      },
      now = Date.now(),
    ) {
      const anchor = startedAt ?? now;
      track(
        "download_success",
        {
          ...base(now),
          delivery_mode: result.deliveryMode,
          ...(result.gatewayRequestId
            ? { gateway_request_id: result.gatewayRequestId }
            : {}),
          filename: result.filename,
          started_at: anchor,
          succeeded_at: now,
          duration_ms: now - anchor,
          ...(result.bytes !== undefined
            ? { bytes_transferred: result.bytes }
            : {}),
        },
        session,
      );
    },
    failed(reason: string, now = Date.now()) {
      const anchor = startedAt ?? now;
      track(
        "download_failed",
        {
          ...base(now),
          reason: sanitizeReason(reason),
          started_at: anchor,
          failed_at: now,
          duration_ms: now - anchor,
        },
        session,
      );
    },
  };
}

export function trackArrived(
  ctx: Omit<AttemptContext, "href" | "anonUserId" | "macArch">,
  session: TrackContext,
  track: Track = defaultTrack,
  now = Date.now(),
): void {
  track(
    "download_success_arrived",
    {
      ...ctx.campaign,
      ...correlation(now),
      os: osLabel(ctx.os),
      arch: ctx.arch,
      place: ctx.place,
      revisit: ctx.revisit,
      auth_state: ctx.authState,
      download_target: "desktop_installer",
      ...audit(now),
    },
    session,
  );
}

export function trackFunnelExit(
  props: DownloadFunnelExitProps,
  session: TrackContext,
  track: Track = defaultTrack,
): void {
  track("download_funnel_exit", props, session);
}

export function trackPageExit(
  props: DownloadPageExitProps,
  session: TrackContext,
  track: Track = defaultTrack,
): void {
  track("download_page_exit", props, session);
}

export function trackRedirectFailed(
  props: Omit<
    DownloadRedirectFailedProps,
    "reason" | "place" | "download_target"
  > & { reason: string },
  session: TrackContext,
  track: Track = defaultTrack,
): void {
  track(
    "download_redirect_failed",
    {
      ...props,
      place: "download-page",
      reason: sanitizeReason(props.reason),
      download_target: "desktop_installer",
    },
    session,
  );
}

/**
 * The Click a landing section sends (source Sites hooks/adapters/useTrackLinkContext useTrackClick):
 * the campaign, the section's place, and `section` or `card` naming what was clicked. Source drops
 * the `event` field for these, so there is none.
 */
export function trackSectionClick(
  props: { place: string; section?: string; card?: string; campaign: Campaign },
  session: TrackContext,
  track: Track = defaultTrack,
  now = Date.now(),
): void {
  track(
    "Click",
    {
      ...props.campaign,
      place: props.place,
      ...(props.section ? { section: props.section } : {}),
      ...(props.card ? { card: props.card } : {}),
      ...audit(now),
    },
    session,
  );
}

/** The Click a download CTA sends; desktop installer clicks also start the click→download join. */
export function trackDownloadClick(
  props: {
    place: string;
    os?: string;
    target: DownloadTarget;
    macArch?: MacArch;
    campaign: Campaign;
  },
  session: TrackContext,
  track: Track = defaultTrack,
  now = Date.now(),
): void {
  const click =
    props.target === "desktop_installer" ? recordClick(now) : undefined;
  track(
    "Click",
    {
      ...props.campaign,
      ...(click ?? {}),
      place: props.place,
      event: "Download",
      ...(props.os ? { os: props.os } : {}),
      download_target: props.target,
      ...(props.macArch && props.target !== "epic"
        ? { mac_arch: props.macArch }
        : {}),
      ...audit(now),
    },
    session,
  );
}
