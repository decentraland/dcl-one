import { BlogIndexView } from "@/pages/blog/BlogIndexView";
import { loadBlogSearch } from "@/data/lib/blog/pages";
import "@dcl/one-ui/foundations/foundations.css";

import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import { blogHeaders, blogResponse, blogSearchMeta } from "../lib/blog";

import type { Route } from "./+types/blog.search";

export const handle = { agentMarkdown: "blogIndex" } satisfies AgentMarkdownHandle;

export const headers = blogHeaders;

export function loader({ request, context }: Route.LoaderArgs) {
  return blogResponse(request, context, loadBlogSearch(request));
}

export function meta({ location }: Route.MetaArgs) {
  return blogSearchMeta(location.search);
}

export default function BlogSearchRoute({ loaderData }: Route.ComponentProps) {
  return <BlogIndexView {...loaderData} />;
}
