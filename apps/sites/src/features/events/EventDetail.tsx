import { useEffect, useRef, type MouseEvent } from "react";

import { EventDetailModal, useEventsCopy, type JumpIn, type RemindControl } from "@dcl/one-ui/apps/sites";

import { track } from "@/core/lib/telemetry/track";
import type { StoryId } from "@/core/lib/telemetry/story-id";
import type { Event } from "@/data/lib/catalyst/places/events";
import type { CreatorProfiles } from "@/data/lib/catalyst/social/creator-profiles";

import { heroWhen, scheduleLine, type Zone } from "../components/event-schedule";
import type { DetectedPlatform } from "../download/options";
import { jumpInFor, mobileLaunch } from "../jump/mobileLaunch";
import type { Reminder } from "./useEventReminders";
import {
  calendarHref,
  eventCreator,
  eventImage,
  eventPageShareUrl,
  eventTarget,
  locationLabel,
  playHref,
  type Target,
} from "./view";

const DETAIL_STORY: StoryId = "admin/whats-on-detail";

/** A plain click opens the detail in place; a modified click keeps the browser's own behaviour. */
export function isPlainClick(event: MouseEvent<HTMLAnchorElement>): boolean {
  return !(
    event.defaultPrevented ||
    event.button !== 0 ||
    event.metaKey ||
    event.ctrlKey ||
    event.shiftKey ||
    event.altKey
  );
}

/** Jump In to an event or place: `/play`, or on a phone the app with the store as its fallback. */
export function eventJumpIn(target: Target, platform: DetectedPlatform): JumpIn {
  const { jumpInHref, onJumpIn } = jumpInFor(playHref(target), mobileLaunch(platform, target));
  return { href: jumpInHref ?? playHref(target), onJumpIn };
}

export function remindControl(reminder: Reminder): RemindControl {
  return { active: reminder.active, pending: reminder.pending, onToggle: reminder.toggle };
}

/**
 * An event's detail dialog, as the events page and a community page open it in place. Both record
 * its views and Jump Ins under the events page's story; without a session id (`sid`, which a client
 * navigation has none of) it records none.
 */
export function EventDetail({
  sid,
  event,
  zone,
  origin,
  pageOrigin,
  jumpIn,
  reminder,
  profiles,
  onClose,
}: {
  profiles: CreatorProfiles;
  sid?: string;
  event: Event;
  zone: Zone;
  origin: string;
  pageOrigin: string;
  jumpIn: (target: Target) => JumpIn;
  reminder?: Reminder;
  onClose: () => void;
}) {
  const { locale, text } = useEventsCopy();
  const target = eventTarget(event);
  const share = eventPageShareUrl(origin, event);
  const viewed = useRef<string | null>(null);
  useEffect(() => {
    if (!sid || viewed.current === event.id) return;
    viewed.current = event.id;
    track("lp_event_viewed", { event_id: event.id, live: event.live }, { sid, story: DETAIL_STORY });
  }, [sid, event.id, event.live]);
  const travel = jumpIn(target);
  const tracked: JumpIn = {
    href: travel.href,
    onJumpIn: (click) => {
      if (sid) {
        track(
          "lp_event_jump_in",
          { event_id: event.id, position: target.realm ? null : `(${target.coordinates.join(",")})` },
          { sid, story: DETAIL_STORY },
        );
      }
      travel.onJumpIn?.(click);
    },
  };
  return (
    <EventDetailModal
      title={event.name ?? text("Untitled event")}
      image={eventImage(event.image, pageOrigin)}
      live={event.live}
      when={event.live ? undefined : heroWhen(event, zone, locale)}
      creator={eventCreator(event, profiles, locale)}
      location={{
        label: locationLabel(target, event.scene_name ?? event.estate_name),
        world: Boolean(target.realm),
        jumpIn: tracked,
      }}
      description={event.description ?? undefined}
      schedule={scheduleLine(event, zone, locale)}
      jumpIn={tracked}
      remind={reminder ? remindControl(reminder) : undefined}
      calendarHref={calendarHref(event, share, undefined, locale)}
      shareUrl={share}
      onClose={onClose}
    />
  );
}
