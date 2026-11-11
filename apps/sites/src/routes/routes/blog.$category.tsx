import { BlogIndexView } from "@/features/blog/BlogIndexView";
import { loadBlogCategory } from "@/data/lib/blog/pages";
import "@dcl/one-ui/foundations/foundations.css";

import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import { blogHeaders, blogListMeta, blogResponse } from "../lib/blog";

import type { Route } from "./+types/blog.$category";

// `/blog/search` and `/blog/author/*` are static routes and win over this one, so a CMS category with
// the slug `search` or `author` could never be reached here: the CMS reads leave those categories out
// (RESERVED_CATEGORY_SLUGS in data/lib/blog/cms.ts) and the tabs never link to them. A slug that is no
// category is looked up as a post, for the `/blog/<post-slug>` links that predate categories in URLs.
export const handle = { agentMarkdown: "blogIndex" } satisfies AgentMarkdownHandle;

export const headers = blogHeaders;

export function loader({ request, params }: Route.LoaderArgs) {
  return blogResponse(request, loadBlogCategory(request, params.category));
}

export function meta({ loaderData }: Route.MetaArgs) {
  return blogListMeta(loaderData);
}

export default function BlogCategoryRoute({ loaderData }: Route.ComponentProps) {
  return <BlogIndexView {...loaderData} />;
}
