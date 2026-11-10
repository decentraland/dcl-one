// Query options for the blog's infinite lists (blog plan R2-1, R3). One options object per scope
// carries the key, the page fetcher, the first page and the next-page rule; the route loader seeding,
// the clientLoader and the hooks all use it. The page fetcher is injected: which upstream it calls
// (owner decision O1) is not decided here.
import {
  infiniteQueryOptions,
  type InfiniteData,
  type QueryKey,
} from "@tanstack/react-query";

import { blogKeys, type BlogListScope } from "./keys";
import { FIRST_PAGE, nextPageParam } from "./normalize";
import type { BlogPage, BlogPageParam } from "./types";

export type FetchBlogPage = (
  scope: BlogListScope,
  param: BlogPageParam,
  signal: AbortSignal,
) => Promise<BlogPage>;

export type BlogListData = InfiniteData<BlogPage, BlogPageParam>;

// One options shape for every blog list (home, category, author, search), so the clientLoader wait
// and the list view take any of them.
const blogInfiniteQuery = (
  queryKey: QueryKey,
  fetch: (param: BlogPageParam, signal: AbortSignal) => Promise<BlogPage>,
  first: BlogPageParam,
  next: (page: BlogPage) => BlogPageParam | undefined,
) =>
  infiniteQueryOptions({
    queryKey,
    queryFn: ({
      pageParam,
      signal,
    }: {
      pageParam: BlogPageParam;
      signal: AbortSignal;
    }) => fetch(pageParam, signal),
    initialPageParam: first,
    getNextPageParam: next,
  });

export type BlogListOptions = ReturnType<typeof blogInfiniteQuery>;

export const blogListQuery = (
  scope: BlogListScope,
  fetchPage: FetchBlogPage,
): BlogListOptions =>
  blogInfiniteQuery(
    blogKeys.list(scope),
    (param, signal) => fetchPage(scope, param, signal),
    FIRST_PAGE,
    nextPageParam,
  );

/** Search pages: 10 hits each, as the source (SearchPage.tsx HITS_PER_PAGE); queries under 3 characters are not sent. */
export const SEARCH_PAGE: BlogPageParam = { skip: 0, limit: 10 };
export const SEARCH_MIN_LENGTH = 3;
/** The search box suggestions: the first 5 hits, at skip 0 (source Search.tsx, hitsPerPage 5, page 0). */
export const SUGGEST_LIMIT = 5;

export type SearchBlogPage = (
  q: string,
  param: BlogPageParam,
  signal: AbortSignal,
) => Promise<BlogPage>;

export const blogSearchQuery = (
  q: string,
  searchPage: SearchBlogPage,
): BlogListOptions =>
  blogInfiniteQuery(
    blogKeys.search(q),
    (param, signal) => searchPage(q, param, signal),
    SEARCH_PAGE,
    (page) =>
      page.skip + page.items.length < page.total
        ? { skip: page.skip + page.items.length, limit: SEARCH_PAGE.limit }
        : undefined,
  );
