import { trackLegacyClick, type LegacySession } from "@/core/lib/telemetry/legacy-click";

// The legacy invite page's `Click` (decentraland/sites `InviteHero` over `useTrackClick`): its call to action
// carried only `data-place`, so the payload is the URL's `utm_*`, that place and the deferred queue's audit fields.

export type InviteHero = "first" | "second";

const PLACE = {
  first: "Referrer Invite First Hero",
  second: "Referrer Invite Second Hero",
} as const;

/** Sends the legacy `Click` for an invite hero's call to action; without a session id, nothing. */
export function trackInviteClick(hero: InviteHero, session: LegacySession): void {
  if (!session.sid) return;
  trackLegacyClick({ place: PLACE[hero] }, session);
}
