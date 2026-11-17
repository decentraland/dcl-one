import { BlogIndexView } from "@/features/blog/BlogIndexView";
import { loadBlogHome } from "@/data/lib/blog/pages";
import "@dcl/one-ui/foundations/foundations.css";

import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import { blogHeaders, blogResponse } from "../lib/blog";

import type { Route } from "./+types/blog._index";

export function meta() {
  return [{ title: "Decentraland Blog" }, { name: "description", content: "News, creator stories, events, and updates from the Decentraland community." }];
}

export const handle = { agentMarkdown: "blogIndex" } satisfies AgentMarkdownHandle;

export const headers = blogHeaders;

export function loader({ request, context }: Route.LoaderArgs) {
  return blogResponse(request, context, loadBlogHome(request));
}

export default function BlogIndexRoute({ loaderData }: Route.ComponentProps) {
  return <BlogIndexView {...loaderData} />;
}
