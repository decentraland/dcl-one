import type { PageView } from "@/core/lib/telemetry/page-view";
import type { BlogListPayload, BlogPostPayload } from "@/data/lib/blog/types";

// The legacy blog's Segment page names and properties. Its names came from the English locale
// (`blog.title`, `search.title`, `search.title_with_query`, `blog.posts_by`); the blog has no locale
// switch here, so the English strings are the names. A missing page (404, CMS down) sends none.

export function blogListPageView(data: BlogListPayload): PageView | null {
  if (data.state !== "ok") return null;
  switch (data.kind) {
    case "home":
      return { name: "Blog", properties: { title: "Blog", path: "/blog" } };
    case "search": {
      const { query } = data;
      return {
        name: query ? "Blog Search Results" : "Blog Search",
        properties: { title: query ? `Search: ${query}` : "Search", hasQuery: query.length > 0, queryLength: query.length },
      };
    }
    case "author": {
      const title = `Posts by ${data.title}`;
      return { name: title, properties: { title, author: data.title, authorSlug: data.authorSlug } };
    }
    case "category":
      return { name: data.title, properties: { title: data.title, categorySlug: data.activeCategory } };
  }
}

export function blogPostPageView(data: BlogPostPayload): PageView | null {
  if (data.state !== "ok") return null;
  const { post } = data;
  return {
    name: post.title,
    properties: {
      title: post.title,
      slug: post.slug,
      category: post.category.title,
      categorySlug: post.category.slug,
      author: post.author.name,
      // A post without an author is the "Decentraland" author, whose slug the legacy site named so.
      authorSlug: post.author.slug || "decentraland",
      publishedDate: data.publishedDate,
    },
  };
}
