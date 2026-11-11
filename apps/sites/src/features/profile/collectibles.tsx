import type { InfiniteData, UseInfiniteQueryResult } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { Link } from "react-router";

import type { CatalogCardLinkProps } from "@dcl/one-ui/apps/sites/provisional/CatalogCard/CatalogCard";
import { ProfileCollectiblesProgress } from "@dcl/one-ui/apps/sites/provisional/profile/ProfileCollectibles";
import { Button } from "@dcl/one-ui/apps/sites/shared/Button/Button";

import type { CollectiblePage } from "@/data/lib/catalyst/marketplace/profile-collectibles";

export type CollectiblesQuery<T> = UseInfiniteQueryResult<InfiniteData<CollectiblePage<T>>, Error>;

/** Opening an item or token is a client navigation, loaded on hover or focus. */
export function CollectibleLink({ to, ...props }: CatalogCardLinkProps) {
  return <Link to={to} prefetch="intent" {...props} />;
}

/**
 * Every loaded page's rows, once each. A row can repeat across pages when a newer one shifts the
 * catalogue's offsets between two reads.
 */
export function loadedRows<T extends { id: string }>(data: InfiniteData<CollectiblePage<T>>): T[] {
  const seen = new Set<string>();
  const rows: T[] = [];
  for (const page of data.pages) {
    for (const row of page.items) {
      if (seen.has(row.id)) continue;
      seen.add(row.id);
      rows.push(row);
    }
  }
  return rows;
}

/** How many the catalogue holds, as the latest page reports it. */
export function catalogueTotal(data: InfiniteData<CollectiblePage<unknown>>): number {
  return data.pages[data.pages.length - 1]?.total ?? 0;
}

/** Under a list: its next page loading, why it failed with a way to retry, Load more, or nothing at the end. */
export function loadMore<T>(query: CollectiblesQuery<T>, noun: string): ReactNode {
  if (query.isFetchingNextPage) return <ProfileCollectiblesProgress label={`Loading more ${noun}`} />;
  if (query.isFetchNextPageError) {
    return (
      <>
        <span>More {noun} could not be loaded.</span>
        <Button variant="secondary" onClick={() => void query.fetchNextPage()}>
          Try again
        </Button>
      </>
    );
  }
  if (!query.hasNextPage) return null;
  return (
    <Button variant="secondary" onClick={() => void query.fetchNextPage()}>
      Load more
    </Button>
  );
}
