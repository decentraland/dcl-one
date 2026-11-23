import { collectCampaignParams, type CampaignParams } from "@/core/lib/telemetry/campaign-params";
import { legacyAnalyticsReady, track, whenLegacyAnalyticsReady } from "@/core/lib/telemetry/track";
import { trackedUserDropped } from "@/core/lib/telemetry/tracked-user";

// The legacy invite page's `Click` (decentraland/sites `InviteHero` over `useTrackClick`): its call to action
// carried only `data-place`, so the payload is the URL's `utm_*`, that place and the deferred queue's audit fields.

export type InviteHero = "first" | "second";

const PLACE = {
  first: "Referrer Invite First Hero",
  second: "Referrer Invite Second Hero",
} as const;

type InviteClickProps = CampaignParams & {
  place: (typeof PLACE)[InviteHero];
  track_called_at: number;
  track_delivered_at: number;
  track_deferred: boolean;
};

/**
 * Sends the legacy `Click` for an invite hero's call to action. It waits for the document's analytics, as the
 * legacy queue did: a bot or an exempt session never sends it.
 */
export function trackInviteClick(hero: InviteHero, { sid, userId }: { sid: string; userId?: string | null }): void {
  try {
    if (!sid) return;
    const calledAt = Date.now();
    const deferred = !legacyAnalyticsReady();
    const base = { ...collectCampaignParams(), place: PLACE[hero] };
    whenLegacyAnalyticsReady(() => {
      const user = userId && !trackedUserDropped() ? userId : undefined;
      const props: InviteClickProps = {
        ...base,
        track_called_at: calledAt,
        track_delivered_at: Date.now(),
        track_deferred: deferred,
      };
      track("Click", props, { sid, ...(user ? { userId: user } : {}), legacy: true });
    });
  } catch {
    // Telemetry never holds the invite's sign-in.
  }
}
