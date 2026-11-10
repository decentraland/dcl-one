// Related posts (blog plan B3): the source's rule (pages/blog/PostPage.tsx, RelatedPost.tsx): the
// first 4 posts of the post's category, minus the post itself, showing at most 3. Loading, an error
// or an empty result shows nothing, as the source does.
import { queryOptions, useQuery } from "@tanstack/react-query";

import { blogKeys } from "@/data/lib/blog/keys";
import { currentBlogSource } from "@/data/lib/blog/current-source";
import type { BlogPost } from "@/data/lib/blog/types";

export const RELATED_SHOWN = 3;
export const RELATED_FETCH_LIMIT = RELATED_SHOWN + 1;

export const relatedQuery = (
  post: Pick<BlogPost, "id" | "slug" | "category">,
) =>
  queryOptions({
    queryKey: blogKeys.related(post.category.slug, post.slug),
    queryFn: async ({ signal }) =>
      (await currentBlogSource()).listPosts(
        { scope: "category", slug: post.category.slug },
        { skip: 0, limit: RELATED_FETCH_LIMIT },
        signal,
      ),
    select: (page) =>
      page.items.filter((item) => item.id !== post.id).slice(0, RELATED_SHOWN),
  });

/** The posts to show, or an empty list while loading, on error, or when there are none. */
export function useRelatedPosts(
  post: Pick<BlogPost, "id" | "slug" | "category">,
): BlogPost[] {
  const { data } = useQuery(relatedQuery(post));
  return data ?? [];
}
