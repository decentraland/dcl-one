import { BlogPostView } from "@/features/blog/BlogPostView";
import { loadBlogPost } from "@/data/lib/blog/pages";
import "@dcl/one-ui/foundations/foundations.css";

import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import { blogHeaders, blogResponse } from "../lib/blog";

import type { Route } from "./+types/blog.$category_.$post";

export const handle = { agentMarkdown: "blogPost" } satisfies AgentMarkdownHandle;

export const headers = blogHeaders;

export function loader({ request, context, params }: Route.LoaderArgs) {
  return blogResponse(request, context, loadBlogPost(request, params.category, params.post));
}

export function meta({ loaderData }: Route.MetaArgs) {
  if (loaderData?.state !== "ok") {
    const title = loaderData?.state === "not-found" ? "Post not found" : "Blog unavailable";
    return [{ title: `${title} | Decentraland Blog` }];
  }
  const { post } = loaderData;
  return [{ title: `${post.title} | Decentraland Blog` }, ...(post.description ? [{ name: "description", content: post.description }] : [])];
}

export default function BlogPostRoute({ loaderData }: Route.ComponentProps) {
  return <BlogPostView {...loaderData} />;
}
