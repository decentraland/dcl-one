import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router";
import { href } from "@/core/lib/router/routes";

import EventDetailDialog, { type EventDialogData } from "@ui/web/events/EventDetailDialog";

import UpstreamUnavailable from "@/features/components/UpstreamUnavailable";
import { CatalystError, postJSON, signedGetJSON } from "@/data/lib/catalyst/client";
import { eventsApiPath } from "@/data/lib/catalyst/typed";
import { fetchEvent, eventCoords, eventJumpUrl, type Event } from "@/data/lib/catalyst/places/events";
import { useAuth } from "@/data/lib/auth/context";
import { openSignIn } from "@/features/components/auth/signin-store";
import { type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { pageLoaderWith } from "@/core/lib/telemetry/page-loader";
import { track } from "@/core/lib/telemetry/track";

import styles from "./whats-on.module.css";

import type { Route } from "./+types/whats-on.$id";
import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import type { StoryId } from "@/core/lib/telemetry/story-id";

export const handle = { agentMarkdown: "eventDetail" } satisfies AgentMarkdownHandle;

const STORY: StoryId = "admin/whats-on-detail";

const FALLBACK: PageDefaults = { variant: "detail_cta" };

export async function loader({ request, context, params }: Route.LoaderArgs) {
  const { id } = params;
  const { sid, wrap, data: { event, unavailable } } = await pageLoaderWith(request, context, FALLBACK, () =>
    fetchEvent(id, { signal: request.signal }).then(
      (event): { event: Event | null; unavailable: boolean } => ({ event, unavailable: false }),
      (err: unknown) => ({ event: null, unavailable: !(err instanceof CatalystError && err.status === 404) }),
    ),
  );
  return wrap({ id, event, sid, unavailable }, { status: event ? 200 : unavailable ? 503 : 404 });
}

/** The events service's weekday mask (Sunday = 1 … Saturday = 64) as weekdays, 0 = Sunday; none when the mask is unset. */
function weekdays(mask: number | null | undefined): number[] | undefined {
  if (!mask) return undefined;
  return [0, 1, 2, 3, 4, 5, 6].filter((day) => (mask & (1 << day)) !== 0);
}

/** The dialog's view of an event, as the reference's normalizeEventEntry maps it. */
export function toDialogEvent(event: Event): EventDialogData {
  const realm = event.world && event.server ? event.server : undefined;
  return {
    id: event.id,
    name: event.name ?? "",
    description: event.description,
    image: event.image,
    x: event.x ?? event.position?.[0] ?? 0,
    y: event.y ?? event.position?.[1] ?? 0,
    creatorAddress: event.user || undefined,
    creatorName: event.user_name || undefined,
    startAt: event.start_at,
    finishAt: event.finish_at,
    recurrent: event.recurrent,
    recurrentFrequency: event.recurrent_frequency,
    recurrentInterval: event.recurrent_interval ?? null,
    recurrentByDay: weekdays(event.recurrent_weekday_mask),
    live: event.live,
    realm,
    isWorld: event.world ?? false,
    placeName: event.scene_name ?? event.estate_name ?? null,
  };
}

type EventEnvelope = { data?: { attending?: boolean } };

/** Whether the signed-in wallet asked to be reminded: the signed event read carries `attending`, as on the reference. */
function useReminder(eventId: string, initial: boolean) {
  const { identity } = useAuth();
  const [reminded, setReminded] = useState(initial);
  const [pending, setPending] = useState(false);
  const path = eventsApiPath("get", "/api/events/{event_id}", { event_id: eventId });

  useEffect(() => {
    if (!identity) return undefined;
    const controller = new AbortController();
    signedGetJSON<EventEnvelope>(path, { identity, signal: controller.signal })
      .then((env) => { if (typeof env?.data?.attending === "boolean") setReminded(env.data.attending); })
      .catch(() => undefined);
    return () => controller.abort();
  }, [identity, path]);

  const toggle = useCallback(() => {
    if (!identity || pending) return;
    const next = !reminded;
    const attendees = eventsApiPath(next ? "post" : "delete", "/api/events/{event_id}/attendees", { event_id: eventId });
    setPending(true);
    postJSON<unknown>(attendees, undefined, { identity, method: next ? "POST" : "DELETE" })
      .then(() => setReminded(next))
      .catch(() => undefined)
      .finally(() => setPending(false));
  }, [eventId, identity, pending, reminded]);

  return { signedIn: !!identity, reminded, pending, toggle: identity ? toggle : undefined };
}

export default function EventDetailRoute({ loaderData }: Route.ComponentProps) {
  const { event, sid, unavailable } = loaderData;
  const navigate = useNavigate();
  const close = useCallback(() => { void navigate(href("/whats-on")); }, [navigate]);

  const viewed = useRef(false);
  useEffect(() => {
    if (viewed.current) return;
    viewed.current = true;
    if (event) track("lp_event_viewed", { event_id: event.id, live: event.live }, { sid, story: STORY });
  }, [event, sid]);

  if (!event) {
    return (
      <div className={styles.detailMissing} role="dialog" aria-modal="true" aria-labelledby="event-detail-missing">
        {unavailable ? (
          <UpstreamUnavailable
            title="Events are temporarily unavailable"
            message="We couldn't reach the events service. Please try again in a moment."
            backHref={href("/whats-on")}
            backLabel="Back to What's On"
          />
        ) : (
          <div className={styles.detailMissingBody}>
            <h1 id="event-detail-missing">Event not found</h1>
            <p>This event may have ended or the link is no longer valid.</p>
            <a href={href("/whats-on")}>&#x2190; Back to What&apos;s On</a>
          </div>
        )}
      </div>
    );
  }
  return <EventDialog event={event} sid={sid} onClose={close} />;
}

function EventDialog({ event, sid, onClose }: { event: Event; sid: string; onClose: () => void }) {
  const reminder = useReminder(event.id, event.attending ?? false);
  const [shareUrl, setShareUrl] = useState(href("/whats-on/:id", { id: event.id }));
  useEffect(() => { setShareUrl(new URL(href("/whats-on/:id", { id: event.id }), window.location.origin).href); }, [event.id]);

  function onJumpIn() {
    track("lp_event_jump_in", { event_id: event.id, position: eventCoords(event) }, { sid, story: STORY });
  }

  return (
    <EventDetailDialog
      event={toDialogEvent(event)}
      signedIn={reminder.signedIn}
      reminded={reminder.reminded}
      remindPending={reminder.pending}
      onToggleRemind={reminder.toggle ?? (() => openSignIn())}
      jumpHref={eventJumpUrl(event) ?? event.url ?? undefined}
      onJumpIn={onJumpIn}
      shareUrl={shareUrl}
      onClose={onClose}
    />
  );
}
