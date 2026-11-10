// The search box's data (source components/blog/Search/Search.tsx): the value debounced by 300 ms;
// the first 5 hits for it once it is 3 characters or more (trimmed), through the same BlogSource as
// every blog read; the previous hits stay while the next ones load, as RTK Query keeps them in source.
import { useEffect, useState } from "react";
import {
  keepPreviousData,
  queryOptions,
  useQuery,
} from "@tanstack/react-query";

import type {
  BlogSearchHit,
  BlogSearchStatus,
} from "@dcl/one-ui/apps/sites/blog/BlogSearch";

import { blogKeys } from "@/data/lib/blog/keys";
import { currentBlogSource } from "@/data/lib/blog/current-source";
import { SEARCH_MIN_LENGTH, SUGGEST_LIMIT } from "@/data/lib/blog/queries";
import type { BlogPost } from "@/data/lib/blog/types";

export const SUGGEST_DEBOUNCE_MS = 300;

export const suggestQuery = (q: string) =>
  queryOptions({
    queryKey: blogKeys.suggest(q),
    queryFn: async ({ signal }) =>
      (await currentBlogSource()).searchPosts(
        q,
        { skip: 0, limit: SUGGEST_LIMIT },
        signal,
      ),
    enabled: q.trim().length >= SEARCH_MIN_LENGTH,
    placeholderData: keepPreviousData,
    select: (page) => page.items.map(toHit),
  });

export const toHit = (post: BlogPost): BlogSearchHit => ({
  id: post.id,
  url: post.url,
  image: post.image.url,
  highlightedTitle: post.highlight?.title ?? post.title,
  highlightedDescription: post.highlight?.description ?? post.description,
});

export function useDebounced<T>(value: T, ms: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => setDebounced(value), ms);
    return () => clearTimeout(timer);
  }, [value, ms]);
  return debounced;
}

/** The search box's value, status and hits. */
export function useBlogSearchBox() {
  const [value, setValue] = useState("");
  const debounced = useDebounced(value, SUGGEST_DEBOUNCE_MS);
  const query = useQuery(suggestQuery(debounced));
  // While the value is long enough but its debounced copy is not yet, source briefly shows "No
  // matches" (its query is skipped); ours shows "Searching..." for that moment (gap table).
  const waiting =
    query.isPending ||
    (value.trim().length >= SEARCH_MIN_LENGTH && !query.isEnabled);
  const status: BlogSearchStatus = query.isError
    ? "error"
    : waiting
      ? "loading"
      : "ready";
  return { value, onValueChange: setValue, status, hits: query.data ?? [] };
}
