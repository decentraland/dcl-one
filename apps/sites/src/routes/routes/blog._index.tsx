import { useEffect, useRef } from "react";

import StBlogHome from "@ui/web/pages/StBlogHome";
import { blogCategories } from "@ui/data/blogCategories";
import "@ui/web/pages/stbloghome.css";

import { blogPostCards, type BlogPostCard } from "@/core/lib/content/blog";
import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import { type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { pageLoader } from "@/core/lib/telemetry/page-loader";
import { track } from "@/core/lib/telemetry/track";

import type { Route } from "./+types/blog._index";
import type { StoryId } from "@/core/lib/telemetry/story-id";

const STORY: StoryId = "misc/blog";

export function meta() {
  return [{ title: "Decentraland Blog" }, { name: "description", content: "News, creator stories, events, and updates from the Decentraland community." }];
}

export const handle = { agentMarkdown: "blogIndex" } satisfies AgentMarkdownHandle;

const FALLBACK: PageDefaults = {
  variant: "index_grid",
};

export async function loader({ request }: Route.LoaderArgs) {
  const url = new URL(request.url);
  const { sid, wrap } = await pageLoader(request,  FALLBACK);

  const category = url.searchParams.get("category") ?? "";
  const posts = blogPostCards(category);

  return wrap({ sid, posts, category });
}

type LoaderData = {
  sid: string;
  posts: BlogPostCard[];
  category: string;
};

export default function BlogIndexRoute({ loaderData }: Route.ComponentProps) {
  const { sid, posts, category } = loaderData as LoaderData;

  const viewed = useRef(false);
  useEffect(() => {
    if (viewed.current) return;
    viewed.current = true;
    track("lp_blog_viewed", { post_count: posts.length }, { sid, story: STORY });
  }, [sid, posts.length]);

  function onPostClick(e: React.MouseEvent<HTMLDivElement>) {
    const anchor = (e.target as HTMLElement).closest("a");
    if (!anchor) return;
    const href = anchor.getAttribute("href") ?? "";
    const m = /^\/blog\/([^/]+)$/.exec(href);
    if (!m) return;
    track("lp_blog_post_clicked", { slug: m[1] }, { sid, story: STORY });
  }

  return (
    <div className="blog-index-route" onClickCapture={onPostClick}>
      <StBlogHome posts={posts} categories={blogCategories(category)} />
    </div>
  );
}
