import { useEffect, useRef } from "react";

import SitesChrome from "@ui/web/frames/SitesChrome";
import { BlogIndexPage, BlogNotFound, BlogRoutingProvider, BlogUnavailable } from "@dcl/one-ui/apps/sites/blog";

import type { BlogListPayload } from "@/data/lib/blog/types";
import { track } from "@/core/lib/telemetry/track";
import type { StoryId } from "@/core/lib/telemetry/story-id";

import { blogRouting, clickedPostSlug } from "./routing";

const STORY: StoryId = "misc/blog";

export type BlogIndexData = { sid: string } & BlogListPayload;

export function BlogIndexView(data: BlogIndexData) {
  const { sid } = data;
  const viewKey = data.state === "ok" ? data.viewKey : null;
  const total = data.state === "ok" ? data.total : 0;
  // Once per list view (the home, a category, an author), as on main where each was a page load.
  // "Load more" and a search keep the view; a search page reports none of its own.
  const viewed = useRef<string | null>(null);
  useEffect(() => {
    if (!viewKey || viewed.current === viewKey) return;
    viewed.current = viewKey;
    track("lp_blog_viewed", { post_count: total }, { sid, story: STORY });
  }, [sid, viewKey, total]);

  function onPostClick(e: React.MouseEvent<HTMLDivElement>) {
    const slug = clickedPostSlug(e);
    if (slug) track("lp_blog_post_clicked", { slug }, { sid, story: STORY });
  }

  return (
    <SitesChrome active="learn">
      <BlogRoutingProvider routing={blogRouting}>
        {data.state === "ok" ? (
          <div onClickCapture={onPostClick}>
            <BlogIndexPage
              categories={data.categories}
              activeCategory={data.activeCategory}
              title={data.title}
              subtitle={data.subtitle}
              query={data.query}
              searched={data.kind === "search"}
              posts={data.posts}
              total={data.total}
              nextHref={data.nextHref}
              emptyText={data.emptyText}
            />
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
