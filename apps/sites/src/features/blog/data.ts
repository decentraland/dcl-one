import {
  blogCategoryList,
  blogPostCards,
  findBlogPost,
  parseBlogQuery,
  parseBlogSort,
  relatedBlogPosts,
  searchBlogPosts,
} from "@/core/lib/content/blog";
import { publicPageUrl } from "@/data/lib/catalyst/bases";

import type { BlogIndexData } from "./BlogIndexView";
import type { BlogPostData } from "./BlogPostView";

/** The index for `?category=`, `?q=` and `?sort=`. */
export function blogIndexData(url: URL): Omit<BlogIndexData, "sid"> {
  const category = url.searchParams.get("category") ?? "";
  const query = parseBlogQuery(url.searchParams.get("q"));
  const sort = parseBlogSort(url.searchParams.get("sort"));
  return {
    posts: searchBlogPosts({ category, query, sort }),
    postCount: blogPostCards(category).length,
    category,
    query,
    sort,
    categories: blogCategoryList(),
  };
}

/** One post, its related posts and its share URL on the public origin; `post` is null for an unknown slug. */
export function blogPostData(request: Request, slug: string): Omit<BlogPostData, "sid"> {
  const post = findBlogPost(slug);
  return {
    slug,
    post,
    related: post ? relatedBlogPosts(post) : [],
    shareUrl: post ? publicPageUrl(request, `/blog/${encodeURIComponent(post.slug)}`) : "",
    categories: blogCategoryList(),
  };
}
