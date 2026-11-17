import { z } from "zod";

import { CatalystError, getJSON } from "../client";
import { dataOf } from "../envelope";

/** The public community list (`GET /v1/communities`) as the /places/communities grid reads it. */

/** One page of the grid; the endpoint caps `limit` at 100. */
export const COMMUNITIES_PAGE_LIMIT = 24;

const PATH = "/v1/communities";

const RowSchema = z.object({
  id: z.string().min(1),
  name: z.string(),
  description: z.string().nullish(),
  ownerName: z.string().nullish(),
  privacy: z.string().nullish(),
  membersCount: z.number().nullish(),
  thumbnailUrl: z.string().nullish(),
});

export type BrowseCommunity = {
  id: string;
  name: string;
  description: string | null;
  ownerName: string | null;
  privacy: "public" | "private";
  membersCount: number | null;
  /** The cover the row names: the node's own upload or a mirrored community's upstream image; null when it has none. */
  thumbnailUrl: string | null;
};

export type CommunitiesPage = {
  rows: BrowseCommunity[];
  /** Rows the endpoint returned, invalid ones included: the page is full when this reaches the limit. */
  received: number;
  total: number;
};

const PageEnvelope = dataOf(z.object({ results: z.array(z.unknown()), total: z.number() }));

/**
 * One page of `GET /v1/communities`, in the node's order (editor's choice, then ranking). Read without a
 * signature, so every visitor gets the same public list. A row without an id or a name is left out; a
 * body that is not a page fails the read, so the page shows its error instead of an empty grid.
 */
function coverOf(value: string | null | undefined): string | null {
  const url = value?.trim() ?? "";
  return url && url !== "N/A" ? url : null;
}

export async function fetchCommunitiesPage(
  params: { offset: number; search: string; limit?: number },
  opts: { signal?: AbortSignal; base?: string } = {},
): Promise<CommunitiesPage> {
  const limit = params.limit ?? COMMUNITIES_PAGE_LIMIT;
  const raw = await getJSON<unknown>(PATH, {
    signal: opts.signal,
    base: opts.base,
    query: { limit, offset: params.offset, ...(params.search ? { search: params.search } : {}) },
  });
  const page = PageEnvelope.safeParse(raw);
  if (!page.success) throw new CatalystError("Catalyst returned an unexpected community page", PATH, 200);
  const rows: BrowseCommunity[] = [];
  for (const node of page.data.data.results) {
    const parsed = RowSchema.safeParse(node);
    if (!parsed.success || !parsed.data.name.trim()) continue;
    const row = parsed.data;
    rows.push({
      id: row.id,
      name: row.name.trim(),
      description: row.description?.trim() || null,
      ownerName: row.ownerName?.trim() || null,
      privacy: row.privacy === "private" ? "private" : "public",
      membersCount: row.membersCount ?? null,
      thumbnailUrl: coverOf(row.thumbnailUrl),
    });
  }
  return { rows, received: page.data.data.results.length, total: page.data.data.total };
}

/** The community's page, as the profile's Communities tab links it. */
export function communityHref(id: string): string {
  return `/landings/community-detail?id=${encodeURIComponent(id)}`;
}
