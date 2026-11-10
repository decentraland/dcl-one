import { BlogPostView } from "@/features/blog/BlogPostView";
import { blogPostData } from "@/features/blog/data";
import "@dcl/one-ui/foundations/foundations.css";

import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import { type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { pageLoader } from "@/core/lib/telemetry/page-loader";

import type { Route } from "./+types/blog.$slug";

export const handle = { agentMarkdown: "blogPost" } satisfies AgentMarkdownHandle;

const FALLBACK: PageDefaults = {
  variant: "index_grid",
};

export async function loader({ request, params }: Route.LoaderArgs) {
  const { sid, wrap } = await pageLoader(request, FALLBACK);
  const payload = { sid, ...blogPostData(request, params.slug ?? "") };
  return wrap(payload, { status: payload.post ? 200 : 404 });
}

export function meta({ loaderData }: Route.MetaArgs) {
  const post = loaderData?.post;
  return [
    { title: post ? `${post.title} | Decentraland Blog` : "Post not found | Decentraland Blog" },
    ...(post ? [{ name: "description", content: post.description }] : []),
  ];
}

export default function BlogPostRoute({ loaderData }: Route.ComponentProps) {
  return <BlogPostView {...loaderData} />;
}
