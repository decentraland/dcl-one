import { useEffect, useRef } from "react";
import { track } from "@/core/lib/telemetry/track";
import { useBlogSession } from "./telemetry";
// The blog's views (blog plan B1): the sub-nav and the post list. Semantic markup only: the styled
// layouts are B4 (library/ui/apps/sites/blog). The route shell with the chrome is routes/lib/blog-page.
import { useInfiniteQuery } from "@tanstack/react-query";

import { BlogPostGrid } from "@dcl/one-ui/apps/sites/blog/BlogPostGrid";
import { BlogSearchResults } from "@dcl/one-ui/apps/sites/blog/BlogSearchResults";

import { searchQuery } from "@/data/lib/blog/options";
import {
  SEARCH_MIN_LENGTH,
  type BlogListOptions,
} from "@/data/lib/blog/queries";

import { toHit } from "./search-box";

export { listQuery, searchQuery } from "@/data/lib/blog/options";

/**
 * An infinite post list (source PostList with useInfiniteBlogPosts): 7, then 6 more each time the
 * reader nears the end; the first post as the desktop hero on home and category lists.
 */
export function PostList({
  options,
  hasMainPost = false,
  onNavigate,
}: {
  options: BlogListOptions;
  hasMainPost?: boolean;
  onNavigate?: (href: string) => void;
}) {
  const query = useInfiniteQuery({
    ...options,
    throwOnError: (_error, query) => !query.state.data,
  });
  const posts = query.data?.pages.flatMap((page) => page.items) ?? [];
  const session = useBlogSession();
  const viewed = useRef(false);
  const isHome = (options.queryKey[2] as { scope?: string })?.scope === "home";
  useEffect(() => {
    if (isHome && query.data && !viewed.current) {
      viewed.current = true;
      track("lp_blog_viewed", { post_count: posts.length }, session);
    }
  }, [isHome, query.data, posts.length, session]);
  // As source: the error copy in place of the list; an empty list renders nothing.
  return (
    <BlogPostGrid
      error={query.isError}
      posts={posts}
      hasMainPost={hasMainPost}
      loading={query.isPending}
      loadingMore={query.isFetchingNextPage}
      hasMore={query.hasNextPage}
      onEndReached={() => void query.fetchNextPage()}
      onNavigate={onNavigate}
    />
  );
}

/**
 * The search page's hits (source SearchPage): 10 per page, more on "Load More", which also scrolls
 * the page down by 500px as source does. Queries under 3 characters are never sent.
 */
export function SearchResultsList({
  q,
  onNavigate,
}: {
  q: string;
  onNavigate?: (href: string) => void;
}) {
  const searchable = q.length >= SEARCH_MIN_LENGTH;
  const query = useInfiniteQuery({
    ...searchQuery(q),
    enabled: searchable,
    throwOnError: (_error, query) => !query.state.data,
  });
  const hits = query.data?.pages.flatMap((page) => page.items.map(toHit)) ?? [];
  return (
    <BlogSearchResults
      q={q}
      searchable={searchable}
      hits={hits}
      loading={searchable && query.isPending}
      loadingMore={query.isFetchingNextPage}
      hasMore={query.hasNextPage}
      onLoadMore={() => {
        void query.fetchNextPage();
        setTimeout(
          () => window.scrollBy({ top: 500, left: 0, behavior: "smooth" }),
          0,
        );
      }}
      onNavigate={onNavigate}
    />
  );
}
