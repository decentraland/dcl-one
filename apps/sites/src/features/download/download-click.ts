import { collectCampaignParams, type CampaignParams } from "@/core/lib/telemetry/campaign-params";
import { sendLegacyDeferred, type LegacyAudit, type LegacySession } from "@/core/lib/telemetry/legacy-click";
import { track } from "@/core/lib/telemetry/track";

import { legacyMacArch, type LegacyMacArch } from "./legacy-mac-arch";

// The legacy site's `Click` for a download call to action (decentraland/sites `useDownloadClick` and
// `useTrackClick` over `buildClickPayload`). Warehouse models read these rows by exact `place` and
// `download_target` values, so every name and value below is the legacy one, byte for byte.

export type DownloadClickPlace =
  | "Landing Hero"
  | "landing-hero-epic"
  | "landing-hero-platform-switch"
  | "Landing Come Hang Out"
  | "come-hang-out"
  | "come-hang-out-platform-switch"
  | "Download"
  | "download-page"
  | "Creators Hero"
  | "Creator Hub Success Footer";

export type DownloadTarget = "desktop_installer" | "epic" | "app_store" | "google_play" | "creator_hub";

export type DownloadClickOs = "Android" | "iOS" | "Windows" | "macOS";

/** The control's own attributes: legacy `data-place`, `data-event` (absent where it was `Click`), `data-os`. */
export type DownloadClick = {
  place: DownloadClickPlace;
  event?: "Download";
  download_target: DownloadTarget;
  os?: DownloadClickOs;
};

type DownloadClickProps = CampaignParams &
  DownloadClick & {
    click_id?: string;
    clicked_at?: number;
    mac_arch?: LegacyMacArch;
  } & LegacyAudit;

export type DownloadClickOptions = {
  /**
   * A `useDownloadClick` control (the default) is a conversion: sent on an exempt session and for a bot, as the
   * legacy beacon did. A `useTrackClick` control (the Creator Hub download page and its success page) is not.
   */
  conversion?: boolean;
  /**
   * Mints the `click_id`/`clicked_at` pair the Explorer funnel on /download_success joins on. Off for the
   * Creator Hub, whose funnel joins on the anonymous id.
   */
  correlate?: boolean;
};

export const DOWNLOAD_CLICK_STORAGE_KEY = "downloadFunnel:lastClick";

type Correlation = { click_id: string; clicked_at: number };

function uuid(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") return crypto.randomUUID();
  return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (char) => {
    const random = Math.floor(Math.random() * 16);
    return (char === "x" ? random : (random & 0x3) | 0x8).toString(16);
  });
}

/** Stored for /download_success; blocked storage loses only that join, never the click. */
function recordCorrelation(): Correlation {
  const correlation = { click_id: uuid(), clicked_at: Date.now() };
  try {
    sessionStorage.setItem(DOWNLOAD_CLICK_STORAGE_KEY, JSON.stringify(correlation));
  } catch {
    // Private mode or a full quota.
  }
  return correlation;
}

export type DownloadClickSession = LegacySession;

/**
 * Sends the legacy `Click` for a download control. Merge order is the legacy one: the URL's `utm_*`, then the
 * correlation, then the control's own attributes.
 *
 * `track_deferred` is the legacy one: true when the click came before the document's analytics was ready (see
 * `legacyAnalyticsReady`). A conversion is sent at once either way, as the legacy beacon was, so its
 * `track_delivered_at` equals `track_called_at`. Any other click waits for readiness, as the legacy queue did:
 * its `track_delivered_at` is when it left, and a bot or an exempt session never sends it.
 */
export function trackDownloadClick(
  click: DownloadClick,
  session: DownloadClickSession,
  { conversion = true, correlate = true }: DownloadClickOptions = {},
): void {
  try {
    const arch = legacyMacArch();
    const base = {
      ...collectCampaignParams(),
      ...(correlate ? recordCorrelation() : {}),
      ...click,
      ...(arch ? { mac_arch: arch } : {}),
    };
    sendLegacyDeferred(
      (audit, ctx) => {
        const props: DownloadClickProps = { ...base, ...audit };
        track("Click", props, ctx);
      },
      session,
      { conversion },
    );
  } catch {
    // Telemetry never breaks a download.
  }
}
