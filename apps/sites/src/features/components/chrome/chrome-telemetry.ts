import {
  track as defaultTrack,
  type TrackContext,
} from "@/core/lib/telemetry/track";

type Track = typeof defaultTrack;

/** What a navbar Click names: the action, and for a link its section and target. */
export type NavbarClick = { action: string; section?: string; href?: string };

/**
 * The Click source's navbar sends (sites LandingNavbar trackNavbar): `Landing Navbar`, `event: click`,
 * the action (`sign_in`, `jump_in`, `logo`, `credits` or a link's i18n key), and a link's section
 * and href, with source's useDeferredTrack delivery fields. There is no readiness queue here: the
 * event is sent as it is called, so it is not deferred.
 */
export function trackNavbarClick(
  click: NavbarClick,
  session: TrackContext,
  track: Track = defaultTrack,
  now = Date.now(),
): void {
  track(
    "Click",
    {
      place: "Landing Navbar",
      event: "click",
      ...click,
      track_called_at: now,
      track_delivered_at: now,
      track_deferred: false,
    },
    session,
  );
}

/** The Click source's footer sends for a social profile link: its platform's name. */
export function trackFooterSocialClick(
  platform: string,
  session: TrackContext,
  track: Track = defaultTrack,
): void {
  track(
    "Click",
    { place: "Landing Footer Social", event: "click", platform },
    session,
  );
}

/** The Click source's footer sends for a Getting started or Resources link: the link's shown label. */
export function trackFooterLinkClick(
  link: string,
  session: TrackContext,
  track: Track = defaultTrack,
): void {
  track(
    "Click",
    { place: "Landing Footer Link", event: "click", link },
    session,
  );
}
