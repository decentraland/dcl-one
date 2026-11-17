import { siteUrl } from "@/data/lib/catalyst/bases";
import type { CommunityInfo, JoinRequest } from "@/data/lib/catalyst/social/community-detail";

export const COMMUNITY_PATH = "/social/communities";

export function communityHref(id: string): string {
  return `${COMMUNITY_PATH}/${encodeURIComponent(id)}`;
}

/** What a sign-in started from this page does once the member is back (`?action=`). */
export type CommunityAction = "join" | "requestToJoin";
export const ACTION_PARAM = "action";
export const NONCE_PARAM = "nonce";

export function readAction(value: string | null): CommunityAction | null {
  return value === "join" || value === "requestToJoin" ? value : null;
}

/**
 * Sign-in returns here with the action the visitor chose and the nonce of the marker this tab wrote
 * for it (`pendingAction.ts`); without a nonce the visitor just comes back to the page.
 */
export function signInReturnHref(id: string, action: CommunityAction, nonce: string | null): string {
  if (!nonce) return communityHref(id);
  const query = new URLSearchParams({ [ACTION_PARAM]: action, [NONCE_PARAM]: nonce });
  return `${communityHref(id)}?${query}`;
}

const THUMBNAIL_PATH = /^\/social\/communities\/[0-9a-f-]{36}\/raw-thumbnail\.png$/i;

/**
 * The community's image. A URL the API names wins: on the communities CDN it is read through this
 * site's asset alias, any other http(s) URL (a mirrored community's) as given. Without one, the
 * image the node keeps for the community, which answers 404 when there is none.
 */
export function thumbnailUrl(community: { id: string; thumbnailUrl: string | null }): string {
  const named = community.thumbnailUrl?.trim();
  if (named && named !== "N/A") {
    try {
      const url = new URL(named);
      if (THUMBNAIL_PATH.test(url.pathname)) return siteUrl(`/assets-cdn${url.pathname}`);
      if (url.protocol === "https:" || url.protocol === "http:") return url.href;
    } catch {
      // Not a URL: the node's own image below.
    }
  }
  return siteUrl(`/assets-cdn/social/communities/${encodeURIComponent(community.id)}/raw-thumbnail.png`);
}

const compact = new Intl.NumberFormat("en-US", { notation: "compact", compactDisplay: "short" });

export function membersLabel(count: number): string {
  return `${compact.format(count)} Members`;
}

export function isMember(community: Pick<CommunityInfo, "role">): boolean {
  return community.role !== null && community.role !== "none";
}

/** A private community shows its members and events to members only. */
export function canViewContent(community: Pick<CommunityInfo, "role" | "privacy">): boolean {
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
  /** A signed read of the member's standing failed; they can try again. */
  | { kind: "error" }
  | { kind: "join" }
  | { kind: "request" }
  | { kind: "cancel"; requestId: string };

export function membershipState(input: {
  signedIn: boolean;
  /** The community read as the member; undefined while that read runs. */
  viewer: Pick<CommunityInfo, "role" | "privacy"> | undefined;
  privacy: CommunityInfo["privacy"];
  /** The member's pending requests; undefined while that read runs. */
  requests: readonly JoinRequest[] | undefined;
  communityId: string;
  /** The member's read of the community failed. */
  viewerFailed?: boolean;
  /** The member's requests read failed. */
  requestsFailed?: boolean;
}): MembershipState {
  if (!input.signedIn) return { kind: "signed-out" };
  if (!input.viewer) return { kind: input.viewerFailed ? "error" : "loading" };
  if (isMember(input.viewer)) return { kind: "joined" };
  if (input.privacy === "public") return { kind: "join" };
  if (!input.requests) return { kind: input.requestsFailed ? "error" : "loading" };
  const pending = pendingRequestFor(input.requests, input.communityId);
  return pending ? { kind: "cancel", requestId: pending.id } : { kind: "request" };
}

/** "owner" -> "Owner", as the members list labels each row. */
export function roleLabel(role: string): string {
  return role ? role.charAt(0).toUpperCase() + role.slice(1).toLowerCase() : "";
}
