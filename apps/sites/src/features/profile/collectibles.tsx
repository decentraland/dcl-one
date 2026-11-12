import type { InfiniteData, UseInfiniteQueryResult } from "@tanstack/react-query";
import { useEffect, useRef, type ReactNode } from "react";
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

/** Whether the catalogue sent any row at all, so no row shown means none could be read, not an empty list. */
export function receivedAny(data: InfiniteData<CollectiblePage<unknown>>): boolean {
  return data.pages.some((page) => page.received > 0);
}

/** How many the catalogue holds, as the latest page reports it. */
export function catalogueTotal(data: InfiniteData<CollectiblePage<unknown>>): number {
  return data.pages[data.pages.length - 1]?.total ?? 0;
}

/**
 * Under a list: one button that stays mounted (so keyboard focus survives a click) reading Load more,
 * or Try again after the next page failed, plus a live region that announces the page loading and
 * its failure. Nothing at the end of the list.
 */
export function loadMore<T>(query: CollectiblesQuery<T>, noun: string, onRequest: () => void): ReactNode {
  const fetching = query.isFetchingNextPage;
  const failed = query.isFetchNextPageError && !fetching;
  if (!query.hasNextPage && !fetching) return null;
  return (
    <>
      <span role="status">
        {fetching ? (
          <ProfileCollectiblesProgress label={`Loading more ${noun}`} />
        ) : failed ? (
          `More ${noun} could not be loaded.`
        ) : null}
      </span>
      <Button
        variant="secondary"
        // Not `disabled`: a disabled button drops the focus it holds.
        aria-disabled={fetching}
        onClick={() => {
          if (fetching) return;
          onRequest();
          void query.fetchNextPage();
        }}
      >
        {failed ? "Try again" : "Load more"}
      </Button>
    </>
  );
}

/**
 * Where keyboard focus goes when the control that held it unmounts: the last Load more (the list
 * ended) or the first read's Try again. A click calls `expectRowsFrom` with the index of the first
 * row it will bring. When the read ends, and only if focus has fallen to the page body, the link of
 * that row takes it; when the read brought no new row (an empty list, only repeated rows, a
 * failure), the panel does. A control that stays mounted keeps the focus. `readAt` changes with
 * every read that ends (TanStack's data and error update times), so a read that ends within one
 * render still counts.
 */
export function useFocusNewRows(rows: number, readAt: number) {
  const listRef = useRef<HTMLUListElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const pending = useRef<number | null>(null);
  useEffect(() => {
    const from = pending.current;
    if (from === null) return;
    pending.current = null;
    const active = document.activeElement;
    if (active && active !== document.body) return;
    const row = listRef.current?.children[from]?.querySelector<HTMLElement>("a, button");
    (row ?? panelRef.current)?.focus();
  }, [rows, readAt]);
  return {
    listRef,
    panelRef,
    expectRowsFrom: (index: number) => {
      pending.current = index;
    },
  };
}
