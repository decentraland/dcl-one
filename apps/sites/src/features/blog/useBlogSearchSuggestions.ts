import { useEffect, useState } from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";

import {
  BLOG_SEARCH_SUGGESTIONS_API,
  type BlogSearchSuggestions,
} from "@dcl/one-ui/apps/sites/blog/blogModel";

import { parseBlogQuery, SEARCH_MIN_LENGTH } from "@/data/lib/blog/normalize";

/** How long typing has to pause before the dropdown asks for suggestions. */
export const SUGGESTIONS_DEBOUNCE_MS = 250;

export const blogSuggestionKeys = {
  query: (q: string) => ["blog", "search-suggestions", q.toLowerCase()] as const,
};

export async function fetchBlogSearchSuggestions(query: string, signal?: AbortSignal): Promise<BlogSearchSuggestions> {
  const response = await fetch(`${BLOG_SEARCH_SUGGESTIONS_API}?q=${encodeURIComponent(query)}`, { signal });
  if (!response.ok) throw new Error(`Blog search suggestions failed: ${response.status}`);
  return (await response.json()) as BlogSearchSuggestions;
}

function useDebounced<T>(value: T, ms: number): T {
  const [settled, setSettled] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => setSettled(value), ms);
    return () => clearTimeout(timer);
  }, [value, ms]);
  return settled;
}

export type BlogSearchSuggestionsState = {
  /** The query the shown suggestions belong to; "" while there are none to show. */
  query: string;
  suggestions: BlogSearchSuggestions | null;
  loading: boolean;
  failed: boolean;
};

/**
 * The search dropdown's suggestions for what the visitor is typing: asked once typing pauses and the
 * query has SEARCH_MIN_LENGTH characters. A response for an older query never replaces a newer one:
 * each query is its own cache entry, and leaving it aborts its request.
 */
export function useBlogSearchSuggestions(input: string, debounceMs = SUGGESTIONS_DEBOUNCE_MS): BlogSearchSuggestionsState {
  const typed = parseBlogQuery(input);
  const query = useDebounced(typed, debounceMs);
  const enabled = query.length >= SEARCH_MIN_LENGTH && typed.length >= SEARCH_MIN_LENGTH;
  const result = useQuery({
    queryKey: blogSuggestionKeys.query(query),
    queryFn: ({ signal }) => fetchBlogSearchSuggestions(query, signal),
    enabled,
    staleTime: 60_000,
    retry: false,
    placeholderData: keepPreviousData,
  });
  if (!enabled) return { query: "", suggestions: null, loading: false, failed: false };
  return {
    query: result.data?.query ?? "",
    suggestions: result.data ?? null,
    loading: result.isFetching || query !== typed,
    failed: result.isError,
  };
}
