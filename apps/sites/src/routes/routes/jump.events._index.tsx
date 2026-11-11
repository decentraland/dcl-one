import "@dcl/one-ui/foundations/foundations.css";
import { JumpCard } from "@dcl/one-ui/apps/sites/jump/JumpCard";
import { JumpEventActions } from "@dcl/one-ui/apps/sites/jump/JumpEventActions";
import { JumpPage } from "@dcl/one-ui/apps/sites/jump/JumpPage";
import { useState } from "react";
import { redirect } from "react-router";
import SitesChrome from "@ui/web/frames/SitesChrome";

import { useAuth } from "@/data/lib/auth/context";
import { siteUrl } from "@/data/lib/catalyst/bases";
import { eventShareUrl, googleCalendarUrl, readJumpTarget, toEventCard } from "@/data/lib/catalyst/jump/jump";
import { loadCreator, loadJumpEvent, loadJumpPlace } from "@/data/lib/catalyst/jump/jump.server";
import { openSignIn } from "@/features/components/auth/signin-store";
import { useLocalWhen } from "@/features/jump/useLocalWhen";
import { useRemindMe } from "@/features/jump/useRemindMe";
import { detectPlatform } from "@/features/download/options";
import { jumpInFor, mobileLaunch } from "@/features/jump/mobileLaunch";
import { cardProps, eventActionState } from "@/features/jump/view";

import type { Route } from "./+types/jump.events._index";

export function meta() {
  return [{ title: "Jump into Decentraland" }];
}

const INVALID = "/jump/events/invalid";
const EVENTS_HREF = "/whats-on";

/** An unknown id, a malformed position or no event at the target lands on the invalid page; a failed read is a 503. */
export async function loader({ request }: Route.LoaderArgs) {
  const target = readJumpTarget(new URL(request.url).searchParams);
  if (!target.validRealm || !target.validId || (!target.id && !target.validPosition)) throw redirect(INVALID);
  const opts = { signal: request.signal };
  const event = await loadJumpEvent(target, opts).catch((error: unknown) => {
    // A visitor who navigated away aborted the read: that is not an outage.
    if (request.signal.aborted) throw error;
    throw new Response("Events are temporarily unavailable", { status: 503 });
  });
  if (!event) throw redirect(INVALID);
  let card = toEventCard(event);
  const [creator, place] = await Promise.all([
    event.user ? loadCreator(event.user, opts).catch(() => null) : null,
    // A live event's occupancy is its scene's; the event record carries none.
    card.event?.live
      ? loadJumpPlace({ lookup: card.coordinates, realm: card.realm }, opts).catch(() => null)
      : null,
  ]);
  if (place?.user_count) card = { ...card, userCount: place.user_count };
  // The public site origin when configured, else the origin this request reached.
  const configured = siteUrl("/");
  const shareUrl = eventShareUrl(/^https?:\/\//.test(configured) ? configured : new URL(request.url).origin, event.id, card);
  return {
    eventId: event.id,
    card: cardProps(card, creator),
    state: eventActionState(card),
    shareUrl,
    calendarHref: card.event
      ? googleCalendarUrl(
          { title: card.title, description: card.description, startAt: card.event.startAt, finishAt: card.event.finishAt },
          shareUrl,
        )
      : null,
    shareText: { title: card.title, text: card.description },
    launch: mobileLaunch(detectPlatform(request.headers.get("user-agent")), card),
  };
}

/** The launch target depends on the visitor's device. */
export function headers() {
  return { Vary: "User-Agent" };
}

export default function JumpEvents({ loaderData }: Route.ComponentProps) {
  const { eventId, card, state, shareUrl, calendarHref, shareText, launch } = loaderData;
  const { identity } = useAuth();
  const remind = useRemindMe(identity, eventId, () => openSignIn({ inPlace: true }), state === "upcoming");
  const [shareNotice, setShareNotice] = useState<string>();

  async function share() {
    if (typeof navigator.share === "function") {
      try {
        await navigator.share({ title: shareText.title, text: shareText.text, url: shareUrl });
        return;
      } catch (error) {
        if (error instanceof DOMException && error.name === "AbortError") return;
      }
    }
    try {
      await navigator.clipboard.writeText(shareUrl);
      setShareNotice("Link copied to clipboard.");
    } catch {
      setShareNotice(`Copy this link to share: ${shareUrl}`);
    }
  }

  const when = useLocalWhen(card.when);

  const actions =
    state === "live" || state === "none" ? undefined : state === "deleted" || state === "ended" ? (
      <JumpEventActions state={state} eventsHref={EVENTS_HREF} />
    ) : (
      <JumpEventActions
        state="upcoming"
        calendarHref={calendarHref ?? undefined}
        remind={{ active: remind.active, pending: remind.pending, onToggle: remind.toggle }}
        onShare={share}
        shareNotice={[remind.failed ? "Couldn't update your reminder. Try again." : null, shareNotice].filter(Boolean).join(" ") || undefined}
      />
    );

  return (
    <SitesChrome active="play">
      <JumpPage>
        <JumpCard {...card} {...jumpInFor(card.jumpInHref, launch)} when={when} actions={actions} />
      </JumpPage>
    </SitesChrome>
  );
}
