import { blogPostHref } from "@dcl/one-ui/apps/sites/blog/blogModel";

import { blogCms, BlogUnavailableError } from "../blog/cms";
import { LATEST_POSTS, type BlogPostSummary } from "./blog-posts";

/** The newest posts from the blog's CMS; null when the CMS is unavailable, so the section stays hidden. */
export async function latestPosts(): Promise<BlogPostSummary[] | null> {
  try {
    const { posts } = await blogCms().list({}, 1);
    return posts.slice(0, LATEST_POSTS).map((post) => ({
      id: post.id,
      title: post.title,
      url: blogPostHref(post.category.slug, post.slug),
      publishedDate: post.publishedAt,
      image: post.image ? { url: post.image.url, width: post.image.width, height: post.image.height } : null,
      category: { slug: post.category.slug, title: post.category.title, url: `/blog/${post.category.slug}` },
    }));
  } catch (error) {
    if (error instanceof BlogUnavailableError) return null;
    throw error;
  }
}
