import { queryOptions } from "@tanstack/react-query";
import { z } from "zod";

import { BLOG_SEARCH_SUGGESTIONS_API, BLOG_SEARCH_SUGGESTIONS_LIMIT, type BlogSearchSuggestions } from "@dcl/one-ui";

const suggestionsSchema: z.ZodType<BlogSearchSuggestions> = z.object({
  query: z.string(),
  total: z.number().int().nonnegative(),
  posts: z.array(z.object({
    slug: z.string(),
    title: z.string(),
    description: z.string(),
    href: z.string().startsWith("/blog/"),
    image: z.object({
      url: z.string(),
      alt: z.string(),
      width: z.number(),
      height: z.number(),
    }).nullable(),
    hue: z.number(),
  })).max(BLOG_SEARCH_SUGGESTIONS_LIMIT),
});

export const blogSuggestionKeys = {
  query: (query: string) => ["blog", "search-suggestions", query.toLowerCase()] as const,
};

async function fetchBlogSearchSuggestions(query: string, signal: AbortSignal) {
  const response = await fetch(`${BLOG_SEARCH_SUGGESTIONS_API}?q=${encodeURIComponent(query)}`, { signal });
  if (!response.ok) {
    await response.body?.cancel();
    throw new Error(`Blog search suggestions failed: ${response.status}`);
  }
  return suggestionsSchema.parse(await response.json());
}

export function blogSuggestionsQuery(query: string) {
  return queryOptions({
    queryKey: blogSuggestionKeys.query(query),
    queryFn: ({ signal }) => fetchBlogSearchSuggestions(query, signal),
    staleTime: 60_000,
    retry: false,
  });
}
