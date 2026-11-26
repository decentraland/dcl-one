import { BlogPostView } from "@/pages/blog/BlogPostView";
import { loadBlogPost } from "@/data/lib/blog/pages";
import "@dcl/one-ui/foundations/foundations.css";

import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import { blogHeaders, blogPostMeta, blogResponse } from "../lib/blog";
import { publicBase } from "../lib/social-meta";

import type { Route } from "./+types/blog.$category_.$post";

export const handle = { agentMarkdown: "blogPost" } satisfies AgentMarkdownHandle;

export const headers = blogHeaders;

export function loader({ request, context, params }: Route.LoaderArgs) {
  return blogResponse(request, context, loadBlogPost(request, params.category, params.post));
}

export function meta({ loaderData, matches, location }: Route.MetaArgs) {
  return blogPostMeta(loaderData, publicBase(matches), location.pathname);
}

export default function BlogPostRoute({ loaderData }: Route.ComponentProps) {
  return <BlogPostView {...loaderData} />;
}
