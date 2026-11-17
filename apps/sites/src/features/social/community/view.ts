import type { UpcomingEventCardProps } from "@dcl/one-ui/apps/sites/events/types";

import { siteUrl } from "@/data/lib/catalyst/bases";
import type { Event } from "@/data/lib/catalyst/places/events";
import type { Community, JoinRequest } from "@/data/lib/catalyst/social/community-detail";
import type { CreatorProfiles } from "@/data/lib/catalyst/social/creator-profiles";

import { calendarHref, eventCreator, eventHref, eventPageShareUrl, relativeStart, type Zone } from "../../events/view";

export const COMMUNITY_PATH = "/social/communities";

export function communityHref(id: string): string {
  return `${COMMUNITY_PATH}/${encodeURIComponent(id)}`;
}

/** What a sign-in started from this page does once the member is back (`?action=`). */
export type CommunityAction = "join" | "requestToJoin";
export const ACTION_PARAM = "action";

export function readAction(value: string | null): CommunityAction | null {
  return value === "join" || value === "requestToJoin" ? value : null;
}

/** Sign-in returns here with the action the visitor chose, which then runs once. */
export function signInReturnHref(id: string, action: CommunityAction): string {
  return `${communityHref(id)}?${ACTION_PARAM}=${action}`;
}

/** The community's own image, served from the node's asset CDN alias. */
export function thumbnailUrl(id: string): string {
  return siteUrl(`/assets-cdn/social/communities/${encodeURIComponent(id)}/raw-thumbnail.png`);
}

const compact = new Intl.NumberFormat("en-US", { notation: "compact", compactDisplay: "short" });

export function membersLabel(count: number): string {
  return `${compact.format(count)} Members`;
}

export function isMember(community: Pick<Community, "role">): boolean {
  return community.role !== null && community.role !== "none";
}

/** A private community shows its members and events to members only. */
export function canViewContent(community: Pick<Community, "role" | "privacy">): boolean {
  return community.privacy === "public" || isMember(community);
}

export function pendingRequestFor(requests: readonly JoinRequest[] | undefined, communityId: string): JoinRequest | undefined {
  return requests?.find((request) => request.communityId === communityId);
}

/** What the member can do here, decided from what is known of them so far. */
export type MembershipState =
  | { kind: "joined" }
  | { kind: "signed-out" }
  | { kind: "loading" }
  | { kind: "join" }
  | { kind: "request" }
  | { kind: "cancel"; requestId: string };

export function membershipState(input: {
  signedIn: boolean;
  /** The community read as the member; undefined while that read runs. */
  viewer: Pick<Community, "role" | "privacy"> | undefined;
  privacy: Community["privacy"];
  /** The member's pending requests; undefined while that read runs. */
  requests: readonly JoinRequest[] | undefined;
  communityId: string;
}): MembershipState {
  if (!input.signedIn) return { kind: "signed-out" };
  if (!input.viewer) return { kind: "loading" };
  if (isMember(input.viewer)) return { kind: "joined" };
  if (input.privacy === "public") return { kind: "join" };
  if (!input.requests) return { kind: "loading" };
  const pending = pendingRequestFor(input.requests, input.communityId);
  return pending ? { kind: "cancel", requestId: pending.id } : { kind: "request" };
}

/** "owner" -> "Owner", as the members list labels each row. */
export function roleLabel(role: string): string {
  return role ? role.charAt(0).toUpperCase() + role.slice(1).toLowerCase() : "";
}

/** A community event as the events page's Upcoming card draws it. */
export function eventCard(
  event: Event,
  ctx: { now: number; zone: Zone; origin: string; profiles: CreatorProfiles },
): UpcomingEventCardProps {
  const start = event.next_start_at ?? event.start_at;
  return {
    id: event.id,
    title: event.name ?? "Untitled event",
    image: event.image,
    href: eventHref(event.id),
    creator: eventCreator(event, ctx.profiles),
    time: start ? { label: relativeStart(start, ctx.now, ctx.zone), dateTime: start } : { label: "Soon" },
    calendarHref: ctx.origin ? calendarHref(event, eventPageShareUrl(ctx.origin, event)) : undefined,
  };
}
