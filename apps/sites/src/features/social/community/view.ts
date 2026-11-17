import { profileFaceUrl } from "@/data/lib/auth/profile-face-url";
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
/** The communities CDN the social API names images on; this site reads them through `/assets-cdn`. */
const THUMBNAIL_HOSTS = new Set(["assets-cdn.decentraland.org", "cdn.decentraland.org"]);

/**
 * The community's image, always from this site's origin. A URL the API names is used only when it is
 * an https image on the communities CDN, read through the `/assets-cdn` alias; anything else (another
 * host, http, not a URL) falls back to the image the node keeps, which answers 404 when there is none.
 */
export function thumbnailUrl(community: { id: string; thumbnailUrl: string | null }): string {
  const named = community.thumbnailUrl?.trim();
  if (named) {
    try {
      const url = new URL(named);
      if (url.protocol === "https:" && THUMBNAIL_HOSTS.has(url.hostname) && THUMBNAIL_PATH.test(url.pathname)) {
        return siteUrl(`/assets-cdn${url.pathname}`);
      }
    } catch {
      // Not a URL: the node's own image below.
    }
  }
  return siteUrl(`/assets-cdn/social/communities/${encodeURIComponent(community.id)}/raw-thumbnail.png`);
}

/** Paths of this site that serve profile faces: the profile image cache and the node's content store. */
const FACE_PATH = /^\/(profile-images\/entities|content\/contents)\//;

/**
 * A member's or owner's face as a path on this site, or undefined (the initial shows instead). Faces
 * come from profile data anyone can write, so no other host is ever requested: an https URL counts
 * only for its path on this site's profile image cache or content store.
 */
export function faceUrl(value: string | undefined | null): string | undefined {
  const local = profileFaceUrl(value ?? undefined, "");
  if (!local) return undefined;
  if (local.startsWith("/") && !local.startsWith("//")) return FACE_PATH.test(local) ? local : undefined;
  try {
    const url = new URL(local);
    return url.protocol === "https:" && FACE_PATH.test(url.pathname) ? url.pathname : undefined;
  } catch {
    return undefined;
  }
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
