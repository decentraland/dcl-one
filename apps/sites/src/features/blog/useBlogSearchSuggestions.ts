import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";

import type { BlogSearchSuggestions } from "@dcl/one-ui/apps/sites/blog/blogModel";

import { parseBlogQuery, SEARCH_MIN_LENGTH } from "@/data/lib/blog/normalize";
import { blogSuggestionsQuery } from "@/data/lib/blog/search-suggestions-query";

/** How long typing has to pause before the dropdown asks for suggestions. */
export const SUGGESTIONS_DEBOUNCE_MS = 250;

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
  const eligible = typed.length >= SEARCH_MIN_LENGTH;
  const enabled = eligible && typed === query;
  const result = useQuery({ ...blogSuggestionsQuery(query), enabled });
  if (!enabled) return { query: "", suggestions: null, loading: eligible, failed: false };
  return {
    query: result.data?.query ?? "",
    suggestions: result.data ?? null,
    loading: result.isFetching || query !== typed,
    failed: result.isError,
  };
}
