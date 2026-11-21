import { BlogIndexView } from "@/pages/blog/BlogIndexView";
import { loadBlogHome } from "@/data/lib/blog/pages";
import { parseBlogPage } from "@/data/lib/blog/normalize";
import "@dcl/one-ui/foundations/foundations.css";

import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import { blogHeaders, blogIndexMeta, blogResponse } from "../lib/blog";
import { publicBase } from "../lib/social-meta";
import type { Route } from "./+types/blog._index";

export function meta({ loaderData, matches, location }: Route.MetaArgs) {
  const page = parseBlogPage(new URLSearchParams(location.search).get("page"));
  return blogIndexMeta(loaderData, publicBase(matches), page > 1 ? `/blog?page=${page}` : "/blog");
}

export const handle = { agentMarkdown: "blogIndex" } satisfies AgentMarkdownHandle;

export const headers = blogHeaders;

export function loader({ request, context }: Route.LoaderArgs) {
  return blogResponse(request, context, loadBlogHome(request));
}

export default function BlogIndexRoute({ loaderData }: Route.ComponentProps) {
  return <BlogIndexView {...loaderData} />;
}
