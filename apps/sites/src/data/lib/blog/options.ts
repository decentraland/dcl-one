import { queryOptions } from "@tanstack/react-query";
import { blogKeys } from "./keys";
import { currentBlogSource } from "./current-source";
import { blogListQuery, blogSearchQuery } from "./queries";

export class BlogMissingError extends Error {
  constructor(public readonly kind: "post" | "category" | "author") {
    super(`The blog ${kind} was not found.`);
  }
}
export const categoriesQuery = () =>
  queryOptions({
    queryKey: blogKeys.categories,
    queryFn: async ({ signal }) =>
      (await currentBlogSource()).listCategories(signal),
  });
export const listQuery = (scope: Parameters<typeof blogListQuery>[0]) =>
  blogListQuery(scope, async (where, page, signal) =>
    (await currentBlogSource()).listPosts(where, page, signal),
  );
export const searchQuery = (q: string) =>
  blogSearchQuery(q, async (text, page, signal) =>
    (await currentBlogSource()).searchPosts(text, page, signal),
  );
export const postQuery = (category: string, slug: string) =>
  queryOptions({
    queryKey: blogKeys.post(category, slug),
    queryFn: async ({ signal }) => {
      const value = await (
        await currentBlogSource()
      ).getPost(category, slug, signal);
      if (!value) throw new BlogMissingError("post");
      return value;
    },
  });
export const authorQuery = (slug: string) =>
  queryOptions({
    queryKey: blogKeys.author(slug),
    queryFn: async ({ signal }) => {
      const value = await (await currentBlogSource()).getAuthor(slug, signal);
      if (!value) throw new BlogMissingError("author");
      return value;
    },
  });
