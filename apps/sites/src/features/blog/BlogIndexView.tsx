import { useEffect, useRef } from "react";

import SitesChrome from "@ui/web/frames/SitesChrome";
import { BlogIndexPage, BlogRoutingProvider } from "@dcl/one-ui/apps/sites/blog";

import type { BlogPostCard, BlogSort } from "@/core/lib/content/blog";
import { track } from "@/core/lib/telemetry/track";
import type { StoryId } from "@/core/lib/telemetry/story-id";

import { blogRouting, clickedPostSlug } from "./routing";

const STORY: StoryId = "misc/blog";

export type BlogIndexData = {
  sid: string;
  posts: BlogPostCard[];
  /** The category's post count, whatever the search: what lp_blog_viewed reports. */
  postCount: number;
  category: string;
  query: string;
  sort: BlogSort;
  categories: { slug: string; title: string }[];
};

export function BlogIndexView({ sid, posts, postCount, category, query, sort, categories }: BlogIndexData) {
  // Once per mounted route: searching, sorting and switching tabs navigate on the client and keep it mounted.
  const viewed = useRef(false);
  useEffect(() => {
    if (viewed.current) return;
    viewed.current = true;
    track("lp_blog_viewed", { post_count: postCount }, { sid, story: STORY });
  }, [sid, postCount]);

  function onPostClick(e: React.MouseEvent<HTMLDivElement>) {
    const slug = clickedPostSlug(e);
    if (slug) track("lp_blog_post_clicked", { slug }, { sid, story: STORY });
  }

  return (
    <SitesChrome active="learn">
      <BlogRoutingProvider routing={blogRouting}>
        <div onClickCapture={onPostClick}>
          <BlogIndexPage categories={categories} category={category} query={query} sort={sort} posts={posts} />
        </div>
      </BlogRoutingProvider>
    </SitesChrome>
  );
}
