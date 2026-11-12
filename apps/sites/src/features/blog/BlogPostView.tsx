import { useEffect, useRef } from "react";

import SitesChrome from "@ui/web/frames/SitesChrome";
import { BlogArticlePage, BlogNotFound, BlogRoutingProvider, BlogUnavailable } from "@dcl/one-ui/apps/sites/blog";

import type { BlogPostPayload } from "@/data/lib/blog/types";
import { track } from "@/core/lib/telemetry/track";
import type { StoryId } from "@/core/lib/telemetry/story-id";

import { blogRouting, clickedPostSlug } from "./routing";

const STORY: StoryId = "misc/blog";

export type BlogPostData = { sid: string } & BlogPostPayload;

export function BlogPostView(data: BlogPostData) {
  const { sid } = data;
  // Once per post: a related card opens the next post on the client, without remounting the route.
  const viewedSlug = useRef<string | null>(null);
  const postSlug = data.state === "ok" ? data.post.slug : null;
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
        {data.state === "ok" ? (
          <div onClickCapture={onPostClick}>
            <BlogArticlePage post={data.post} shareUrl={data.shareUrl} related={data.related} categories={data.categories} />
          </div>
        ) : data.state === "not-found" ? (
          <BlogNotFound kind={data.kind} />
        ) : (
          <BlogUnavailable />
        )}
      </BlogRoutingProvider>
    </SitesChrome>
  );
}
