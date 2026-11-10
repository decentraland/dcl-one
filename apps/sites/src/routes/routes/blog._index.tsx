// /blog: the latest posts (blog plan B1, R7). Owner decision O6: the mock ?category= filter is not
// carried over.
import { BlogErrorBoundary, blogHeaders } from "../lib/blog-page";
import {
  BlogHome,
  homeClientLoader,
  homeLoader,
  type HomeData,
} from "../lib/blog-home";
import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";

import type { Route } from "./+types/blog._index";

// Agents asking for markdown (Accept: text/markdown, ?format=md) get the first page as markdown.
export const handle = {
  agentMarkdown: "blogIndex",
} satisfies AgentMarkdownHandle;

export const loader = ({ request }: Route.LoaderArgs) =>
  homeLoader({ request });

export const clientLoader = homeClientLoader;

export const meta = () => [];

export default function BlogIndexRoute({ loaderData }: Route.ComponentProps) {
  return <BlogHome data={loaderData as HomeData} />;
}

export const ErrorBoundary = BlogErrorBoundary;

export const headers = blogHeaders;
