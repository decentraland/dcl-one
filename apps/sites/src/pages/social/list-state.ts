import type { RefObject } from "react";

import type { CommunityListMore, CommunityListStatus } from "@dcl/one-ui/apps/sites";

type PagedQuery = {
  isPending: boolean;
  isError: boolean;
  hasNextPage: boolean;
  isFetchingNextPage: boolean;
  isFetchNextPageError: boolean;
};

export function listStatus(query: Pick<PagedQuery, "isPending" | "isError">): CommunityListStatus {
  return query.isPending ? "pending" : query.isError ? "error" : "ready";
}

/** A failed next page is offered again by hand; otherwise the sentinel reads it, or Load more without IntersectionObserver. */
export function listMore(
  query: Omit<PagedQuery, "isPending" | "isError">,
  loader: { ref: RefObject<HTMLDivElement | null>; manual: boolean },
  onLoadMore: () => void,
): CommunityListMore {
  const state = query.isFetchNextPageError
    ? "error"
    : !query.hasNextPage
      ? "none"
      : query.isFetchingNextPage
        ? "loading"
        : "idle";
  return { state, manual: loader.manual, sentinelRef: loader.ref, onLoadMore };
}
