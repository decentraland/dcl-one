import { renderToString } from "react-dom/server";
import { beforeEach, describe, expect, it } from "vitest";
import { createFixtureBlogSource } from "@/data/lib/blog/fixture-source";
import type { BlogPost } from "@/data/lib/blog/types";
import { categorySeo, postSeo } from "@/features/blog/seo";
import { BlogHead } from "./blog-head";

describe("when route-owned blog metadata renders on the server", () => {
  let post: BlogPost;
  beforeEach(async () => {
    post = (await createFixtureBlogSource().getPost(
      "announcements",
      "decentralands-2024-recap",
      new AbortController().signal,
    ))!;
  });
  it("should emit one source title, description and canonical for an article", () => {
    const html = renderToString(
      <BlogHead seo={postSeo(post, "https://sites.example")} />,
    );
    expect(html.match(/<title>/g)).toHaveLength(1);
    expect(html).toContain("Decentraland’s 2024 Recap | Decentraland</title>");
    expect(html.match(/name="description"/g)).toHaveLength(1);
    expect(html.match(/rel="canonical"/g)).toHaveLength(1);
    expect(html).toContain(
      'href="https://sites.example/blog/announcements/decentralands-2024-recap"',
    );
    expect(html).toContain('property="article:published_time"');
  });
  it("should keep category metadata free of article fields and mark an error head noindex", () => {
    const html = renderToString(
      <BlogHead
        seo={categorySeo(post.category, "https://sites.example")}
        noIndex
      />,
    );
    expect(html).toContain("Announcements | Decentraland</title>");
    expect(html).not.toContain('property="article:');
    expect(html).toContain('name="robots" content="noindex"');
  });
});
