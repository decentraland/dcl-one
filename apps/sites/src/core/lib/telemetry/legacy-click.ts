import { collectCampaignParams } from "./campaign-params";
import { legacyAnalyticsReady, track, whenLegacyAnalyticsReady, type TrackContext } from "./track";
import { trackedUserDropped } from "./tracked-user";

/** The legacy site's `useDeferredTrack` audit fields. */
export type LegacyAudit = { track_called_at: number; track_delivered_at: number; track_deferred: boolean };

/** Who acted: the session id, and the server-verified wallet when the visitor is signed in. */
export type LegacySession = { sid: string; userId?: string | null };

/**
 * The legacy `useDeferredTrack` transport. `track_deferred` is true when the call came before the document's
 * analytics was ready (`legacyAnalyticsReady`). A conversion leaves at once, as the legacy beacon did; anything
 * else waits for readiness, so a bot or an exempt session never sends it. `send` builds the event, so the
 * contract harvests each caller's literal name and shape.
 */
export function sendLegacyDeferred(
  send: (audit: LegacyAudit, ctx: TrackContext) => void,
  { sid, userId }: LegacySession,
  { conversion = false }: { conversion?: boolean } = {},
): void {
  const calledAt = Date.now();
  const deferred = !legacyAnalyticsReady();
  const fire = () => {
    // A sign-out in progress leaves the account out before root's data has reloaded without it.
    const user = userId && !trackedUserDropped() ? userId : undefined;
    send(
      { track_called_at: calledAt, track_delivered_at: Date.now(), track_deferred: deferred },
      { sid, ...(user ? { userId: user } : {}), legacy: true, ...(conversion ? { conversion: true } : {}) },
    );
  };
  if (conversion) fire();
  else whenLegacyAnalyticsReady(fire);
}

/** The legacy `data-place` of a non-download control (decentraland/sites `SectionViewedTrack` values). */
export type LegacyClickPlace =
  | "Landing Catch The Vibe"
  | "Landing Explore"
  | "Landing Weekly Rituals"
  | "Creators Hero"
  | "Creators Why"
  | "Creators Create"
  | "Creators Live Scenes"
  | "Creators Connect"
  | "Creators Learn"
  | "Creators Blog"
  | "Creators Faqs";

/** The control's other `data-*` attributes; an empty one was never sent. */
export type LegacyClick = { place: LegacyClickPlace; section?: string; card?: string; tab?: string; title?: string };

/**
 * The legacy `useTrackClick` `Click` of a control that downloads nothing: the URL's `utm_*`, then the control's
 * attributes (its `data-event` was `Click`, which the legacy payload dropped), then the audit fields.
 */
export function trackLegacyClick(click: LegacyClick, session: LegacySession): void {
  try {
    const attributes = Object.fromEntries(
      Object.entries(click).filter(([, value]) => value !== undefined && value !== ""),
    ) as LegacyClick;
    const base = { ...collectCampaignParams(), ...attributes };
    sendLegacyDeferred((audit, ctx) => track("Click", { ...base, ...audit }, ctx), session);
  } catch {
    // Telemetry never breaks a click.
  }
}

/** The /create sections that sent `Section Viewed` (the only page that did). */
export type LegacySectionViewed =
  | "Creators Why"
  | "Creators Create"
  | "Creators Live Scenes"
  | "Creators Connect"
  | "Creators Learn"
  | "Creators Blog"
  | "Creators Faqs";

/** decentraland-ui2's `useMobileMediaQuery` breakpoint, which the legacy `mobile` flag read. */
const LEGACY_MOBILE_QUERY = "(max-width: 767.95px)";

/** The legacy `Section Viewed`: the deferred transport, no `utm_*`. */
export function trackSectionViewed(section: LegacySectionViewed, session: LegacySession): void {
  try {
    const mobile = typeof window.matchMedia === "function" && window.matchMedia(LEGACY_MOBILE_QUERY).matches;
    sendLegacyDeferred(
      (audit, ctx) => track("Section Viewed", { section_viewed: section, mobile, ...audit }, ctx),
      session,
    );
  } catch {
    // Telemetry never breaks a page.
  }
}
