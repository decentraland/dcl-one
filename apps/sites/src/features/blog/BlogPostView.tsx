import { useEffect, useRef } from "react";

import SitesChrome from "@ui/web/frames/SitesChrome";
import { BlogArticlePage, BlogNotFound, BlogRoutingProvider, type BlogRelatedCard } from "@dcl/one-ui/apps/sites/blog";

import { blogArticle, type BlogPost } from "@/core/lib/content/blog";
import { track } from "@/core/lib/telemetry/track";
import type { StoryId } from "@/core/lib/telemetry/story-id";

import { blogRouting, clickedPostSlug } from "./routing";

const STORY: StoryId = "misc/blog";

export type BlogPostData = {
  sid: string;
  slug: string;
  post: BlogPost | null;
  related: BlogRelatedCard[];
  shareUrl: string;
  categories: { slug: string; title: string }[];
};

export function BlogPostView({ sid, slug, post, related, shareUrl, categories }: BlogPostData) {
  // Once per post: a related card opens the next post on the client, without remounting the route.
  const viewedSlug = useRef<string | null>(null);
  const postSlug = post?.slug ?? null;
  useEffect(() => {
    if (!postSlug || viewedSlug.current === postSlug) return;
    viewedSlug.current = postSlug;
    track("lp_blog_post_viewed", { slug: postSlug }, { sid, story: STORY });
  }, [postSlug, sid]);

  function onPostClick(e: React.MouseEvent<HTMLDivElement>) {
    const clicked = clickedPostSlug(e);
    if (clicked) track("lp_blog_post_clicked", { slug: clicked }, { sid, story: STORY });
  }

  return (
    <SitesChrome active="learn">
      <BlogRoutingProvider routing={blogRouting}>
        {post ? (
          <div onClickCapture={onPostClick}>
            <BlogArticlePage post={blogArticle(post)} shareUrl={shareUrl} related={related} categories={categories} />
          </div>
        ) : (
          <BlogNotFound slug={slug} />
        )}
      </BlogRoutingProvider>
    </SitesChrome>
  );
}
