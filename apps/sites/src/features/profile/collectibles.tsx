import type { InfiniteData, UseInfiniteQueryResult } from "@tanstack/react-query";
import type { MouseEvent, ReactNode } from "react";
import { Link, useNavigate } from "react-router";
import {
  Button,
  type ShopCardCreator,
  type ShopCardLinkProps,
} from "@dcl/one-ui";
import {
  CreationCard as CreationView,
  ProfileLoadMore as LoadMore,
  type CollectibleLinkProps,
  type CollectiblesState,
  type FocusRows,
} from "@dcl/one-ui/apps/sites";

import { assetPage } from "@/core/lib/shop/redirects";
import type { CollectiblePage, ProfileCreation } from "@/data/lib/catalyst/marketplace/profile-collectibles";

export type CollectiblesQuery<T> = UseInfiniteQueryResult<InfiniteData<CollectiblePage<T>>, Error>;

/** Opening an item or token is a client navigation, loaded on hover or focus. */
export function CollectibleLink({ to, ...props }: CollectibleLinkProps) {
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

/** Whether the catalogue sent any row at all, so no row shown means none could be read, not an empty list. */
export function receivedAny(data: InfiniteData<CollectiblePage<unknown>>): boolean {
  return data.pages.some((page) => page.received > 0);
}

/** How many the catalogue holds, as the latest page reports it. */
export function catalogueTotal(data: InfiniteData<CollectiblePage<unknown>>): number {
  return data.pages[data.pages.length - 1]?.total ?? 0;
}

export function loadMore<T>(query: CollectiblesQuery<T>, noun: string, onRequest: () => void): ReactNode {
  const fetching = query.isFetchingNextPage;
  if (!query.hasNextPage && !fetching) return null;
  return (
    <LoadMore
      noun={noun}
      fetching={fetching}
      failed={query.isFetchNextPageError && !fetching}
      onRequest={() => {
        onRequest();
        void query.fetchNextPage();
      }}
    />
  );
}
/** Try again for a read that ended without rows to show: read every loaded page again. */
function tryAgain<T>(query: CollectiblesQuery<T>, focus: FocusRows): ReactNode {
  return (
    <Button
      variant="secondary"
      size="md"
      // A refetch with data already read keeps the query a success: the button shows it is working.
      loading={query.isFetching}
      onClick={() => {
        focus.retryAgain();
        void query.refetch();
      }}
    >
      Try again
    </Button>
  );
}

/**
 * The state of a list before it has rows to show: the first read failed (with no data yet, a Try
 * again refetch puts the query back to pending, so it shows as loading), is loading, or brought a
 * page whose every row failed to read (not an empty list; Load more when later pages may still read,
 * Try again otherwise). `null` once there are rows, or none to show.
 */
export function unreadyState<T extends { id: string }>(
  query: CollectiblesQuery<T>,
  rows: readonly T[],
  copy: { noun: string; unavailable: string; unreadable: string },
  focus: FocusRows,
): CollectiblesState | null {
  if (query.isError && !query.data) {
    return { kind: "error", message: copy.unavailable, action: tryAgain(query, focus) };
  }
  if (!query.data) return { kind: "loading", label: `Loading ${copy.noun}` };
  if (rows.length === 0 && receivedAny(query.data)) {
    // The status line says when the reader's Try again is reading and when it failed; a refetch they
    // did not ask for (e.g. on window focus) keeps the first result's copy.
    const message =
      query.isRefetching && focus.retry === "reading"
        ? `Reading the ${copy.noun} again.`
        : query.isRefetchError && focus.retry === "done"
          ? `${copy.unreadable} Trying again failed.`
          : copy.unreadable;
    return {
      kind: "error",
      message,
      action: loadMore(query, copy.noun, () => focus.expectRowsFrom(0)) ?? tryAgain(query, focus),
    };
  }
  return null;
}

/**
 * Card actions are links into the Shop and Create, which this router serves: a plain click
 * navigates in place, as the card's title does; a modified or middle click keeps the browser's own.
 */
export function useRouterClick(): (href: string) => (event: MouseEvent<HTMLAnchorElement>) => void {
  const navigate = useNavigate();
  return (href) => (event) => {
    if (
      event.defaultPrevented ||
      event.button !== 0 ||
      event.metaKey ||
      event.ctrlKey ||
      event.shiftKey ||
      event.altKey
    ) {
      return;
    }
    event.preventDefault();
    void navigate(href);
  };
}

/** The public ShopCard's link: this router's, loaded on hover or focus. */
export function ShopCardRouterLink({ href, ...props }: ShopCardLinkProps) {
  return <Link to={href} prefetch="intent" {...props} />;
}

export function CreationCard({
  item,
  creator,
  actions,
}: {
  item: ProfileCreation;
  creator?: ShopCardCreator;
  actions?: boolean;
}) {
  const navigate = useRouterClick();
  const href = assetPage(item.contractAddress, item.itemId, "item");
  return (
    <CreationView
      item={item}
      creator={creator}
      actions={actions}
      href={href}
      onNavigate={navigate(href)}
      linkComponent={ShopCardRouterLink}
    />
  );
}
