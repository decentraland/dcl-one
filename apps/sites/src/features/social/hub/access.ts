import { CatalystError } from "@/data/lib/catalyst/client";
import type { HubCommunity, HubMember } from "@/data/lib/catalyst/social/community-hub";

const MANAGER_ROLES: readonly string[] = ["owner", "moderator", "mod"];
const READER_ROLES: readonly string[] = [...MANAGER_ROLES, "member"];

/** The reader belongs to the community (any role but none). */
export const isMember = (community: Pick<HubCommunity, "role">): boolean => community.role !== null;

/** Owners and moderators publish announcements, decide requests, invite and manage members. */
export const manages = (role: string | null | undefined): boolean => MANAGER_ROLES.includes(role ?? "");

/** Announcements are public in a public community and for members in a private one; never for a banned reader. */
export function canReadPosts(community: HubCommunity | null | undefined): boolean {
  if (!community || community.role === "banned") return false;
  return community.privacy !== "private" || READER_ROLES.includes(community.role ?? "");
}

/**
 * Whether the reader may change `member`: an owner manages moderators and members, a moderator
 * manages members, and nobody manages themselves.
 */
export function canManageMember(role: string | null | undefined, viewer: string | null, member: HubMember): boolean {
  if (!viewer || viewer.toLowerCase() === member.memberAddress.toLowerCase()) return false;
  const target = member.role === "mod" ? "moderator" : member.role;
  if (role === "owner") return target === "moderator" || target === "member";
  return manages(role) && target === "member";
}

const refused = (error: unknown) => error instanceof CatalystError && (error.status === 401 || error.status === 403);

/** What to tell the member about a failed request: the server's own words, else `fallback`. */
export function failureText(error: unknown, fallback: string, refusal?: string): string {
  if (refusal && refused(error)) return refusal;
  if (error instanceof CatalystError) return error.serverMessage ? error.message : fallback;
  return error instanceof Error && error.message ? error.message : fallback;
}
