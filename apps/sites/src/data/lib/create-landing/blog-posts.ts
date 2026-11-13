import { queryOptions } from "@tanstack/react-query";

/** How many posts the landing's blog rail shows (decentraland.org /create: FromTheBlog, cms.discovery POSTS_LIMIT). */
export const LATEST_POSTS = 3;

/** A post as the landing's blog card shows it. */
export type BlogPostSummary = {
  id: string;
  title: string;
  url: string;
  publishedDate: string;
  image: { url: string; width: number; height: number } | null;
  category: { slug: string; title: string; url: string };
};

/** The newest posts, from the landing's own resource route; null when the list is unavailable, so the section stays hidden. */
export async function fetchLatestPosts(signal?: AbortSignal): Promise<BlogPostSummary[] | null> {
  const res = await fetch("/create/latest-posts", { signal, headers: { accept: "application/json" } });
  if (!res.ok) return null;
  const body = (await res.json()) as unknown;
  return Array.isArray(body) ? (body as BlogPostSummary[]) : null;
}

export const latestPostsQuery = () =>
  queryOptions({
    queryKey: ["public", "blog", "latest"] as const,
    queryFn: ({ signal }) => fetchLatestPosts(signal),
    staleTime: 60_000,
  });
