import { z } from "zod";

import type { AuthIdentity } from "../../auth/types";
import { CatalystError, getJSON, postJSON, signedGetJSON } from "../client";

/**
 * `/social`: the communities hub. Its lists, a community's channels (members, shared places,
 * announcements) and the membership requests, read from the social API's `/v1` routes. A signed-in
 * member reads them signed, so each row carries their role; a visitor reads them anonymously.
 */

const text = z
  .string()
  .nullish()
  .transform((value) => value ?? null);
const count = z.number().catch(0);

export const HubCommunitySchema = z.object({
  id: z.string(),
  name: z.string(),
  description: text,
  thumbnailUrl: text,
  visibility: text,
  privacy: z
    .string()
    .nullish()
    .transform((value): "public" | "private" => (value === "private" ? "private" : "public")),
  isLive: z
    .boolean()
    .nullish()
    .transform((value) => value === true),
  membersCount: count,
  /** The signed-in member's role, lower case; null for a visitor or a community they are not in. */
  role: z
    .string()
    .nullish()
    .transform((value) => {
      const role = value?.toLowerCase();
      return role && role !== "none" ? role : null;
    }),
  ownerName: text,
});
export type HubCommunity = z.infer<typeof HubCommunitySchema>;

const HubMemberSchema = z.object({
  memberAddress: z.string(),
  name: text,
  role: z
    .string()
    .nullish()
    .transform((value) => value?.toLowerCase() || "member"),
  profilePictureUrl: text,
  hasClaimedName: z
    .boolean()
    .nullish()
    .transform((value) => value === true),
});
export type HubMember = z.infer<typeof HubMemberSchema>;

/** A place the community shares, as the places API describes it: a world by name, a scene by its base parcel. */
const HubPlaceSchema = z
  .object({
    id: z.string(),
    title: text,
    description: text,
    image: text,
    base_position: text,
    world: z.boolean().nullish(),
    world_name: text,
    addedBy: text,
    addedAt: text,
  })
  .transform(({ base_position, world, world_name, ...place }) => ({
    ...place,
    location: (world ? world_name : base_position) ?? null,
  }));
export type HubPlace = z.infer<typeof HubPlaceSchema>;

const HubPostSchema = z.object({
  id: z.string(),
  authorAddress: z.string(),
  authorName: text,
  authorProfilePictureUrl: text,
  authorHasClaimedName: z
    .boolean()
    .nullish()
    .transform((value) => value === true),
  content: z.string().catch(""),
  createdAt: text,
  likesCount: count,
  isLikedByUser: z
    .boolean()
    .nullish()
    .transform((value) => value === true),
});
export type HubPost = z.infer<typeof HubPostSchema>;

const HubRequestSchema = z.object({
  id: z.string(),
  communityId: z.string(),
  memberAddress: z.string().catch(""),
  type: z.enum(["invite", "request_to_join"]),
  status: z.string(),
  name: text,
});
export type HubRequest = z.infer<typeof HubRequestSchema>;
export type HubRequestDecision = "accepted" | "rejected" | "cancelled";

export type HubPage<T> = { rows: T[]; total: number; received: number };

/** A community as the hub opens it: a missing one is `community: null`; a closed private one has no members or places. */
export type HubDetail = {
  id: string;
  community: HubCommunity | null;
  members: HubMember[];
  places: HubPlace[];
};

export const HUB_PAGE_SIZE = 100;
export const HUB_POSTS_PAGE_SIZE = 50;

type ReadOptions = { identity: AuthIdentity | null; signal?: AbortSignal };

const communityPath = (id: string, rest = "") => `/v1/communities/${encodeURIComponent(id)}${rest}`;

/** The `data` envelope the social API answers with, or the body itself where a route has none. */
function payload(raw: unknown): unknown {
  return raw !== null && typeof raw === "object" && "data" in raw ? (raw as { data: unknown }).data : raw;
}

async function read(path: string, { identity, signal }: ReadOptions): Promise<unknown> {
  const raw = identity ? await signedGetJSON<unknown>(path, { identity, signal }) : await getJSON<unknown>(path, { signal });
  return payload(raw);
}

/** The rows that validate; `received` counts every row the page held, which is what paging advances by. */
function page<T>(raw: unknown, key: string, schema: z.ZodType<T, unknown>, url: string): HubPage<T> {
  const body = z.object({ total: z.number().optional() }).catchall(z.unknown()).safeParse(raw);
  const list = body.success ? body.data[key] : undefined;
  if (!Array.isArray(list)) throw new CatalystError("Community list failed validation", url);
  const rows = list.flatMap((row) => {
    const parsed = schema.safeParse(row);
    return parsed.success ? [parsed.data] : [];
  });
  return { rows, total: body.data?.total ?? list.length, received: list.length };
}

/** One page of the communities the reader can discover, optionally matching `search`. */
export async function fetchHubCommunities(
  opts: ReadOptions & { offset: number; search?: string },
): Promise<HubPage<HubCommunity>> {
  const query = `?limit=${HUB_PAGE_SIZE}&offset=${opts.offset}${opts.search ? `&search=${encodeURIComponent(opts.search)}` : ""}`;
  const url = `/v1/communities${query}`;
  return page(await read(url, opts), "results", HubCommunitySchema, url);
}

/**
 * The community with its members and shared places. A community the reader may not open in full
 * (refused or unknown to the signed read) falls back to its discovery row, so a private one still
 * shows what anyone can see; without that row it is `community: null`.
 */
export async function fetchHubDetail(id: string, opts: ReadOptions): Promise<HubDetail> {
  const empty = { id, members: [], places: [] };
  let community: HubCommunity | null;
  try {
    const parsed = HubCommunitySchema.safeParse(await read(communityPath(id), opts));
    if (!parsed.success) throw new CatalystError("Community failed validation", communityPath(id));
    community = parsed.data;
  } catch (error) {
    if (!(error instanceof CatalystError) || ![401, 403, 404].includes(error.status ?? 0)) throw error;
    const url = `/v1/communities?communityId=${encodeURIComponent(id)}&limit=1`;
    const summary = page(await read(url, opts), "results", HubCommunitySchema, url);
    community = summary.rows.find((row) => row.id === id) ?? null;
  }
  if (!community) return { ...empty, community: null };
  if (community.privacy === "private" && !community.role) return { ...empty, community };
  const [members, places] = await Promise.all([
    read(communityPath(id, `/members?limit=${HUB_PAGE_SIZE}`), opts),
    read(communityPath(id, "/places"), opts),
  ]);
  return {
    id,
    community,
    members: page(members, "results", HubMemberSchema, communityPath(id, "/members")).rows,
    places: page(places, "results", HubPlaceSchema, communityPath(id, "/places")).rows,
  };
}

/** A page of the community's announcements, newest first as the API answers. */
export async function fetchHubPosts(
  id: string,
  opts: ReadOptions & { offset: number; limit: number },
): Promise<HubPage<HubPost>> {
  const url = communityPath(id, `/posts?limit=${opts.limit}&offset=${opts.offset}`);
  return page(await read(url, opts), "posts", HubPostSchema, url);
}

/** Every request at `path`, read 100 at a time. */
async function fetchAllRequests(path: string, opts: ReadOptions): Promise<HubRequest[]> {
  const rows: HubRequest[] = [];
  let offset = 0;
  for (;;) {
    const url = `${path}?limit=${HUB_PAGE_SIZE}&offset=${offset}`;
    const next = page(await read(url, opts), "results", HubRequestSchema, url);
    rows.push(...next.rows);
    offset += next.received;
    if (offset >= next.total || next.received === 0) return rows;
  }
}

/** The member's own invitations and requests to join. */
export function fetchOwnRequests(identity: AuthIdentity, signal?: AbortSignal): Promise<HubRequest[]> {
  return fetchAllRequests(`/v1/members/${encodeURIComponent(identity.signer.toLowerCase())}/requests`, { identity, signal });
}

/** The requests a community's owner or moderators decide on. */
export function fetchCommunityRequests(identity: AuthIdentity, id: string, signal?: AbortSignal): Promise<HubRequest[]> {
  return fetchAllRequests(communityPath(id, "/requests"), { identity, signal });
}

/** Leaves the community as the signed-in member. */
export async function leaveCommunity(identity: AuthIdentity, id: string): Promise<void> {
  await postJSON<unknown>(communityPath(id, `/members/${encodeURIComponent(identity.signer.toLowerCase())}`), undefined, {
    identity,
    method: "DELETE",
  });
}

/** Accepts, rejects or withdraws an invitation or a request to join. */
export async function decideRequest(
  identity: AuthIdentity,
  request: Pick<HubRequest, "id" | "communityId">,
  intention: HubRequestDecision,
): Promise<void> {
  await postJSON<unknown>(communityPath(request.communityId, `/requests/${encodeURIComponent(request.id)}`), { intention }, {
    identity,
    method: "PATCH",
  });
}

/** Invites a wallet to the community. */
export async function inviteMember(identity: AuthIdentity, id: string, address: string): Promise<void> {
  await postJSON<unknown>(communityPath(id, "/requests"), { type: "invite", targetedAddress: address }, { identity });
}

/** Makes a member a moderator or a member again, or removes them. */
export async function changeMember(
  identity: AuthIdentity,
  id: string,
  address: string,
  change: "moderator" | "member" | "remove",
): Promise<void> {
  const path = communityPath(id, `/members/${encodeURIComponent(address)}`);
  if (change === "remove") await postJSON<unknown>(path, undefined, { identity, method: "DELETE" });
  else await postJSON<unknown>(path, { role: change }, { identity, method: "PATCH" });
}

/** Publishes an announcement. The key makes a retry after a lost answer publish it once. */
export async function publishPost(identity: AuthIdentity, id: string, content: string, idempotencyKey: string): Promise<void> {
  await postJSON<unknown>(communityPath(id, "/posts"), { content }, { identity, headers: { "Idempotency-Key": idempotencyKey } });
}

export async function editPost(identity: AuthIdentity, id: string, postId: string, content: string): Promise<void> {
  await postJSON<unknown>(communityPath(id, `/posts/${encodeURIComponent(postId)}`), { content }, { identity, method: "PATCH" });
}

export async function deletePost(identity: AuthIdentity, id: string, postId: string): Promise<void> {
  await postJSON<unknown>(communityPath(id, `/posts/${encodeURIComponent(postId)}`), undefined, { identity, method: "DELETE" });
}

export async function likePost(identity: AuthIdentity, id: string, postId: string, like: boolean): Promise<void> {
  await postJSON<unknown>(communityPath(id, `/posts/${encodeURIComponent(postId)}/like`), undefined, {
    identity,
    method: like ? "POST" : "DELETE",
  });
}
