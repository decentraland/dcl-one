// Query keys for the blog (blog plan R1-2). Lists and search hold InfiniteData<BlogPage, BlogPageParam>.
export type BlogListScope =
  | { scope: "home" }
  | { scope: "category"; slug: string }
  | { scope: "author"; slug: string };

export const blogKeys = {
  all: ["blog"] as const,
  list: (scope: BlogListScope) => ["blog", "list", scope] as const,
  search: (q: string) => ["blog", "search", { q }] as const,
  /** The search box suggestions: the first 5 hits (source Search.tsx, hitsPerPage 5). */
  suggest: (q: string) => ["blog", "suggest", { q }] as const,
  post: (category: string, slug: string) =>
    ["blog", "post", { category, slug }] as const,
  related: (category: string, excludeSlug: string) =>
    ["blog", "related", { category, excludeSlug }] as const,
  categories: ["blog", "categories"] as const,
  category: (slug: string) => ["blog", "category", { slug }] as const,
  author: (slug: string) => ["blog", "author", { slug }] as const,
};
