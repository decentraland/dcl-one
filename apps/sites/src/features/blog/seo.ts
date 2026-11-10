import type { BlogAuthor, BlogCategory, BlogPost } from "@/data/lib/blog/types";
import {
  BLOG_DEFAULT_DESCRIPTION,
  blogCanonical,
  type BlogSeo,
} from "./route-kit";

export const homeSeo = (base: string): BlogSeo => ({
  title: "Decentraland Blog",
  description: BLOG_DEFAULT_DESCRIPTION,
  canonical: blogCanonical(base, "/blog"),
});
export const categorySeo = (category: BlogCategory, base: string): BlogSeo => ({
  title: category.title,
  description: category.description,
  canonical: blogCanonical(base, category.url),
  image: category.image,
});
export const authorSeo = (author: BlogAuthor, base: string): BlogSeo => ({
  title: author.title,
  description: author.description,
  canonical: blogCanonical(base, author.url),
  type: "profile",
  image: author.image,
});
export const postSeo = (post: BlogPost, base: string): BlogSeo => ({
  title: post.title,
  description: post.description,
  canonical: blogCanonical(base, post.url),
  type: "article",
  image: post.image,
  article: {
    author: post.author.title,
    publishedTime: post.publishedDate,
    section: post.category.title,
  },
});
export const searchSeo = (q: string, base: string): BlogSeo => ({
  title: q ? `Search: ${q}` : "Search",
  description: "Search the Decentraland Blog",
  canonical: blogCanonical(
    base,
    `/blog/search${q ? "?" + new URLSearchParams({ q }) : ""}`,
  ),
});
