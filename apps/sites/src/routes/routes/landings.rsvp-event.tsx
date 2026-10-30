
import LdRsvpEventView from "@ui/landings/pages/LdRsvpEventView";

import {
  eventCoords,
  eventJumpUrl,
  formatEventWhen,
} from "@/data/lib/catalyst/places/events";
import { loadRsvp } from "@/data/lib/catalyst/landings/rsvp.server";
import { buildRsvpCommit } from "@/data/lib/catalyst/landings/rsvp";
import { useAuth } from "@/data/lib/auth/context";
import { getIdentity } from "@/data/lib/auth/session";
import { type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { pageLoader } from "@/core/lib/telemetry/page-loader";

import { openSignIn } from "@/features/components/auth/signin-store";

import RsvpWizard, {
  type RsvpEventView,
} from "@/features/social/rsvp-event/RsvpWizard";

import type { Route } from "./+types/landings.rsvp-event";
import type { StoryId } from "@/core/lib/telemetry/story-id";

const STORY: StoryId = "landings/rsvp-event";

const FALLBACK: PageDefaults = {
  variant: "confirm",
};

export async function loader({ request }: Route.LoaderArgs) {
  const url = new URL(request.url);
  const step = url.searchParams.get("step")?.trim() || null;
  const requestedId = url.searchParams.get("event")?.trim() || null;

  const { sid, assignment, wrap } = await pageLoader(
    request,
    
    FALLBACK,
  );

  const rsvp = await loadRsvp(requestedId, { signal: request.signal });
  const eventId = rsvp.event?.id ?? requestedId ?? "";

  const payload = {
    sid,
    step,
    eventId,
    assignment,
    rsvp,
  };
  return wrap(payload);
}

type LoaderData = Awaited<ReturnType<typeof loader>> extends Response
  ? never
  : {
      sid: string;
      step: string | null;
      eventId: string;
      assignment: PageDefaults;
      rsvp: Awaited<ReturnType<typeof loadRsvp>>;
    };

export default function LandingsRsvpEvent({ loaderData }: Route.ComponentProps) {
  const { sid, step, eventId, assignment, rsvp } = loaderData as LoaderData;
  const e = rsvp.event;

  const auth = useAuth();
  const commit = buildRsvpCommit(getIdentity);

  const view: RsvpEventView = {
    id: eventId,
    title: e?.name ?? "Event not found",
    when: formatEventWhen(e?.start_at ?? e?.next_start_at ?? null),
    host: e?.user_name ?? e?.scene_name ?? "Decentraland",
    description: e?.description ?? "This event may have ended or the link is no longer valid.",
    schedule: formatEventWhen(e?.start_at ?? e?.next_start_at ?? null),
    location: (e ? eventCoords(e) : null) ?? "Location not published",
    jumpHref:
      e?.url ??
      (e ? eventJumpUrl(e) : null) ??
      `/whats-on/${encodeURIComponent(eventId)}`,
  };

  return (
    <LdRsvpEventView>
      <RsvpWizard
        trackCtx={{
          sid,
          story: STORY,
          variant: assignment.variant,
        }}
        event={view}
        count={rsvp.count}
        initialStep={step ?? undefined}
        commit={commit}
        signedIn={auth.isConnected}
        onSignIn={openSignIn}
      />
    </LdRsvpEventView>
  );
}
