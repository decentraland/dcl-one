import { z } from "zod";

import type { AuthIdentity } from "../../auth/types";
import { CatalystError, getJSON, postJSON, signedGetJSON } from "../client";
import { fetchEvents, type Event } from "../places/events";

/**
 * `/social/communities/:id`: one community, its members, its events and the signed-in member's
 * standing in it, read from the social API's `/v1` routes (the ones that carry the members' names
 * and faces) and the events API. Client-safe: the browser reads the same routes.
 */

const nullableStr = z
  .string()
  .nullish()
  .transform((value) => value ?? null);

export const CommunityInfoSchema = z.object({
  id: z.string(),
  name: z.string(),
  description: nullableStr,
  ownerAddress: z.string(),
  ownerName: nullableStr,
  privacy: z.enum(["public", "private"]),
  membersCount: z.number(),
  /** The image the API names, when it names one (a list row, a mirrored community). */
  thumbnailUrl: nullableStr,
  /** The signed-in member's role; absent on an anonymous read. */
  role: z
    .enum(["owner", "moderator", "member", "none"])
    .nullish()
    .catch(null)
    .transform((value) => value ?? null),
});
export type CommunityInfo = z.infer<typeof CommunityInfoSchema>;

const MemberSchema = z.object({
  memberAddress: z.string(),
  role: z.string(),
  name: nullableStr,
  profilePictureUrl: nullableStr,
  /** Absent when the node could not resolve the profile: no verified mark is shown. */
  hasClaimedName: z.boolean().nullish(),
});
export type CommunityMember = z.infer<typeof MemberSchema>;

const MembersPageSchema = z.object({
  data: z.object({
    results: z.array(z.unknown()),
    total: z.number(),
    page: z.number().optional(),
    pages: z.number().optional(),
  }),
});

export type MembersPage = { members: CommunityMember[]; total: number; hasMore: boolean };

const RequestSchema = z.object({
  id: z.string(),
  communityId: z.string(),
  type: z.string(),
  status: z.string(),
});
export type JoinRequest = z.infer<typeof RequestSchema>;

const RequestsSchema = z.object({ data: z.object({ results: z.array(z.unknown()) }) });

export type EventsPage = { events: Event[]; total: number };

/** The ids the social API accepts; anything else is a page that cannot exist. */
const COMMUNITY_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export function isCommunityId(id: string): boolean {
  return COMMUNITY_ID.test(id);
}

/** The community does not exist: the route answers 404. */
export class CommunityNotFoundError extends Error {
  readonly id: string;
  constructor(id: string) {
    super(`Community not found: ${id}`);
    this.name = "CommunityNotFoundError";
    this.id = id;
  }
}

const path = (id: string, rest = "") => `/v1/communities/${encodeURIComponent(id)}${rest}`;

function parseCommunity(raw: unknown, url: string): CommunityInfo {
  const parsed = z.object({ data: CommunityInfoSchema }).safeParse(raw);
  if (!parsed.success) throw new CatalystError("Community failed validation", url);
  return parsed.data.data;
}

/**
 * The community as anyone sees it, or, signed, with the member's role. A malformed id and an
 * unknown one are not found; any other failure throws `CatalystError` (unavailable).
 */
export async function fetchCommunity(
  id: string,
  opts: { identity?: AuthIdentity | null; signal?: AbortSignal } = {},
): Promise<CommunityInfo> {
  if (!isCommunityId(id)) throw new CommunityNotFoundError(id);
  try {
    const raw = opts.identity
      ? await signedGetJSON<unknown>(path(id), { identity: opts.identity, signal: opts.signal })
      : await getJSON<unknown>(path(id), { signal: opts.signal });
    return parseCommunity(raw, path(id));
  } catch (error) {
    if (error instanceof CatalystError && (error.status === 404 || error.status === 400)) {
      throw new CommunityNotFoundError(id);
    }
    throw error;
  }
}

export const MEMBERS_PAGE_SIZE = 10;
export const EVENTS_PAGE_SIZE = 12;

/** One page of members; a private community's members are read signed, as one of them. */
export async function fetchCommunityMembers(
  id: string,
  offset: number,
  opts: { identity?: AuthIdentity | null; signal?: AbortSignal } = {},
): Promise<MembersPage> {
  const query = { limit: MEMBERS_PAGE_SIZE, offset };
  const raw = opts.identity
    ? await signedGetJSON<unknown>(path(id, "/members"), { identity: opts.identity, query, signal: opts.signal })
    : await getJSON<unknown>(path(id, "/members"), { query, signal: opts.signal });
  const page = MembersPageSchema.safeParse(raw);
  if (!page.success) throw new CatalystError("Community members failed validation", path(id, "/members"));
  const members: CommunityMember[] = [];
  for (const row of page.data.data.results) {
    const member = MemberSchema.safeParse(row);
    if (member.success) members.push(member.data);
  }
  const { total, page: current, pages } = page.data.data;
  const hasMore = current !== undefined && pages !== undefined ? current < pages : offset + MEMBERS_PAGE_SIZE < total;
  return { members, total, hasMore };
}

/** One page of the events the community hosts. */
export async function fetchCommunityEvents(
  id: string,
  offset: number,
  opts: { signal?: AbortSignal } = {},
): Promise<EventsPage> {
  const { data, total } = await fetchEvents(
    { community_id: id, limit: EVENTS_PAGE_SIZE, offset },
    { signal: opts.signal },
  );
  return { events: data, total };
}

/** The member's own pending requests to join a private community. */
export async function fetchJoinRequests(identity: AuthIdentity, opts: { signal?: AbortSignal } = {}): Promise<JoinRequest[]> {
  const url = `/v1/members/${encodeURIComponent(identity.signer.toLowerCase())}/requests`;
  const raw = await signedGetJSON<unknown>(url, {
    identity,
    query: { type: "request_to_join" },
    signal: opts.signal,
  });
  const parsed = RequestsSchema.safeParse(raw);
  if (!parsed.success) throw new CatalystError("Community requests failed validation", url);
  const requests: JoinRequest[] = [];
  for (const row of parsed.data.data.results) {
    const request = RequestSchema.safeParse(row);
    if (request.success && request.data.status === "pending" && request.data.type === "request_to_join") {
      requests.push(request.data);
    }
  }
  return requests;
}

/** Joins a public community as the signed-in member. */
export async function joinCommunity(identity: AuthIdentity, id: string): Promise<void> {
  await postJSON<unknown>(path(id, "/members"), undefined, { identity });
}

/** Asks to join a private community; the reply is the new pending request. */
export async function requestToJoin(identity: AuthIdentity, id: string): Promise<JoinRequest> {
  const raw = await postJSON<unknown>(
    path(id, "/requests"),
    { targetedAddress: identity.signer.toLowerCase(), type: "request_to_join" },
    { identity },
  );
  const parsed = z.object({ data: RequestSchema }).safeParse(raw);
  if (!parsed.success) throw new CatalystError("Community request failed validation", path(id, "/requests"));
  return parsed.data.data;
}

/** Withdraws the member's pending request. */
export async function cancelJoinRequest(identity: AuthIdentity, id: string, requestId: string): Promise<void> {
  await postJSON<unknown>(
    path(id, `/requests/${encodeURIComponent(requestId)}`),
    { intention: "cancelled" },
    { identity, method: "PATCH" },
  );
}
