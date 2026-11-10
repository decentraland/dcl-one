import { useEffect, useRef } from "react";

import SitesChrome from "@ui/web/frames/SitesChrome";
import { BlogArticlePage, BlogNotFound } from "@dcl/one-ui/apps/sites/blog";
import "@dcl/one-ui/foundations/foundations.css";

import {
  blogCategoryList,
  blogDateToIso,
  blogReadMinutes,
  findBlogPost,
  relatedBlogPosts,
} from "@/core/lib/content/blog";
import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import { type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { pageLoader } from "@/core/lib/telemetry/page-loader";
import { track } from "@/core/lib/telemetry/track";

import type { Route } from "./+types/blog.$slug";
import type { StoryId } from "@/core/lib/telemetry/story-id";

const STORY: StoryId = "misc/blog";

export const handle = { agentMarkdown: "blogPost" } satisfies AgentMarkdownHandle;

const FALLBACK: PageDefaults = {
  variant: "index_grid",
};

export async function loader({ request, params }: Route.LoaderArgs) {
  const slug = params.slug ?? "";

  const { sid, wrap } = await pageLoader(
    request,
    
    FALLBACK,
  );

  const post = findBlogPost(slug);
  const related = post ? relatedBlogPosts(post) : [];
  // The address this request was served under, so a shared link points at this deployment.
  const shareUrl = post ? new URL(`/blog/${encodeURIComponent(post.slug)}`, request.url).href : "";

  const payload = { slug, post, related, sid, shareUrl, categories: blogCategoryList() };
  return wrap(payload, { status: post ? 200 : 404 });
}

export function meta({ loaderData }: Route.MetaArgs) {
  const post = loaderData?.post;
  return [
    { title: post ? `${post.title} | Decentraland Blog` : "Post not found | Decentraland Blog" },
    ...(post ? [{ name: "description", content: post.description }] : []),
  ];
}

export default function BlogPostRoute({ loaderData }: Route.ComponentProps) {
  const { slug, post, related, sid, shareUrl, categories } = loaderData;

  const viewed = useRef(false);
  useEffect(() => {
    if (viewed.current) return;
    viewed.current = true;
    if (post) {
      track("lp_blog_post_viewed", { slug: post.slug }, { sid, story: STORY });
    }
  }, [post, sid]);

  if (!post) {
    return (
      <SitesChrome active="learn">
        <BlogNotFound slug={slug} />
      </SitesChrome>
    );
  }

  return (
    <SitesChrome active="learn">
      <BlogArticlePage
        post={{
          slug: post.slug,
          title: post.title,
          description: post.description,
          publishedAt: blogDateToIso(post.publishedDate),
          readMinutes: blogReadMinutes(post.body),
          category: { slug: post.category.slug, title: post.category.title },
          author: post.author.title,
          hue: post.hue,
          body: post.body,
        }}
        shareUrl={shareUrl}
        related={related}
        categories={categories}
      />
    </SitesChrome>
  );
}
