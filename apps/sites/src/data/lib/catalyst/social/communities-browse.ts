import { infiniteQueryOptions } from "@tanstack/react-query";

import { COMMUNITIES_PAGE_LIMIT, fetchCommunitiesPage, type BrowseCommunity, type CommunitiesPage } from "./communities-list";

/** The /places/communities grid. Client-safe: the loader and the page share these factories. */

export const COMMUNITIES_STALE_MS = 30_000;
/** A shorter search is not sent, so one or two keystrokes do not refetch the whole list (as the places search). */
export const MIN_COMMUNITY_SEARCH_LENGTH = 3;

/** The search the grid sends, or "" for none. */
export function effectiveCommunitySearch(raw: string | null | undefined): string {
  const value = raw?.trim() ?? "";
  return value.length >= MIN_COMMUNITY_SEARCH_LENGTH ? value : "";
}

/** The offset after `page`, or undefined once the list is drained: a short page ends it even when `total` says otherwise. */
export function nextCommunitiesOffset(page: CommunitiesPage, offset: number, limit = COMMUNITIES_PAGE_LIMIT): number | undefined {
  const next = offset + page.received;
  return page.received < limit || next >= page.total ? undefined : next;
}

export const communitiesKeys = {
  all: ["communities"] as const,
  browse: (search: string) => ["communities", "browse", search] as const,
};

/** The grid: one infinite query per search. */
export const browseCommunitiesQuery = (search: string) =>
  infiniteQueryOptions({
    queryKey: communitiesKeys.browse(search),
    staleTime: COMMUNITIES_STALE_MS,
    initialPageParam: 0,
    queryFn: ({ pageParam, signal }) => fetchCommunitiesPage({ offset: pageParam, search }, { signal }),
    getNextPageParam: (last, _pages, offset) => nextCommunitiesOffset(last, offset),
  });

/** The rows of every loaded page, a community repeated by an overlapping page kept once. */
export function communityRows(pages: readonly CommunitiesPage[]): BrowseCommunity[] {
  const seen = new Set<string>();
  const out: BrowseCommunity[] = [];
  for (const page of pages) {
    for (const row of page.rows) {
      if (seen.has(row.id)) continue;
      seen.add(row.id);
      out.push(row);
    }
  }
  return out;
}
