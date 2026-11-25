import { z } from "zod";

import type { AuthIdentity } from "../../auth/types";
import { CatalystError, getJSON, signedGetJSON } from "../client";

/**
 * The communities a profile lists, from two reads of the social API. Client-safe: the browser reads
 * the same routes.
 *
 * - `/v1/members/:address/communities`, unsigned: the member's public, listed communities, each with
 *   its id, its name and the member's role, and nothing else.
 * - `/v1/communities?onlyMemberOf=true`, signed by the member: every community of the signer, private
 *   and unlisted ones too, with the members count, the privacy and the picture the API names.
 */
export type ProfileCommunity = {
  id: string;
  name: string;
  role: "owner" | "moderator" | "member";
  /** Only the member's own signed list carries the count and the privacy. */
  membersCount: number | null;
  privacy: "public" | "private" | null;
  /** The picture the API names, when it names one; otherwise the page reads the image by id. */
  thumbnailUrl: string | null;
};

/** `total` is the node's count: more than the rows when the member has more than one page. */
export type ProfileCommunities = { communities: ProfileCommunity[]; total: number };

/** One request; the profile shows no second page. */
export const PROFILE_COMMUNITIES_LIMIT = 100;

const PageSchema = z.object({ data: z.object({ results: z.array(z.unknown()), total: z.number() }) });

const RowSchema = z.object({
  id: z.string().min(1),
  name: z.string().trim().min(1),
  role: z.string().nullish(),
  membersCount: z.number().nullish(),
  privacy: z.enum(["public", "private"]).nullish(),
  thumbnailUrl: z.string().nullish(),
});

function roleOf(value: string | null | undefined): ProfileCommunity["role"] {
  return value === "owner" || value === "moderator" ? value : "member";
}

/** A page that is not the API's envelope is a failure; a row that is not a community is left out. */
function readPage(raw: unknown, url: string): ProfileCommunities {
  const page = PageSchema.safeParse(raw);
  if (!page.success) throw new CatalystError("Profile communities failed validation", url);
  const communities: ProfileCommunity[] = [];
  for (const result of page.data.data.results) {
    const row = RowSchema.safeParse(result);
    if (!row.success) continue;
    communities.push({
      id: row.data.id,
      name: row.data.name,
      role: roleOf(row.data.role),
      membersCount: row.data.membersCount ?? null,
      privacy: row.data.privacy ?? null,
      thumbnailUrl: row.data.thumbnailUrl ?? null,
    });
  }
  return { communities, total: page.data.data.total };
}

export async function fetchMemberCommunities(
  address: string,
  opts: { signal?: AbortSignal } = {},
): Promise<ProfileCommunities> {
  const url = `/v1/members/${encodeURIComponent(address.toLowerCase())}/communities`;
  const raw = await getJSON<unknown>(url, { query: { limit: PROFILE_COMMUNITIES_LIMIT }, signal: opts.signal });
  return readPage(raw, url);
}

export async function fetchMyCommunities(
  identity: AuthIdentity,
  opts: { signal?: AbortSignal } = {},
): Promise<ProfileCommunities> {
  const url = "/v1/communities";
  const raw = await signedGetJSON<unknown>(url, {
    identity,
    query: { onlyMemberOf: true, limit: PROFILE_COMMUNITIES_LIMIT },
    signal: opts.signal,
  });
  return readPage(raw, url);
}
