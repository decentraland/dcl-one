import { BlogIndexView } from "@/features/blog/BlogIndexView";
import { loadBlogSearch } from "@/data/lib/blog/pages";
import "@dcl/one-ui/foundations/foundations.css";

import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import { blogHeaders, blogListMeta, blogResponse } from "../lib/blog";

import type { Route } from "./+types/blog.search";

export const handle = { agentMarkdown: "blogIndex" } satisfies AgentMarkdownHandle;

export const headers = blogHeaders;

export function loader({ request }: Route.LoaderArgs) {
  return blogResponse(request, loadBlogSearch(request));
}

export function meta({ loaderData }: Route.MetaArgs) {
  return blogListMeta(loaderData);
}

export default function BlogSearchRoute({ loaderData }: Route.ComponentProps) {
  return <BlogIndexView {...loaderData} />;
}
