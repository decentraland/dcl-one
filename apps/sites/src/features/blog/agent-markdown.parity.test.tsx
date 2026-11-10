// @vitest-environment jsdom
// The post markdown for agents against the HTML renderer, on the 11 sampled posts: no heading, link,
// image or visible text the renderer shows is lost (both use the one allowlist in library/ui blog/links.ts).
import "../locale/test-setup";
import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { BlogRichText } from "@dcl/one-ui/apps/sites/blog/BlogRichText";

import {
  blogArticleToMarkdown,
  blogBodyToMarkdown,
} from "@/data/lib/agent/blog-richtext";
import { createFixtureBlogSource } from "@/data/lib/blog/fixture-source";
import oracle from "@/data/lib/blog/fixtures/oracle-bodies.json";
import type { BlogPost } from "@/data/lib/blog/types";

const BASE = "https://decentraland.org/blog";
const keys = Object.keys(
  (oracle as { bodies: Record<string, unknown> }).bodies,
);
const flat = (value: string) => value.replace(/\s+/g, " ").trim();

async function post(key: string): Promise<BlogPost> {
  const [category, slug] = key.split("/") as [string, string];
  return (await createFixtureBlogSource().getPost(
    category,
    slug,
    new AbortController().signal,
  ))!;
}

function rendered(body: BlogPost) {
  const { container } = render(
    <BlogRichText
      body={body.body}
      assets={body.bodyAssets}
      blogBaseUrl={BASE}
    />,
  );
  // X embeds hold only the widget's loading text, not post content.
  container
    .querySelectorAll("[data-tweet-id]")
    .forEach((tweet) => tweet.remove());
  return container;
}

describe("the post markdown against the HTML renderer", () => {
  it.each(keys)(
    "keeps every heading, link, image and text of %s",
    async (key) => {
      const item = await post(key);
      const html = rendered(item);
      const md = blogBodyToMarkdown(item.body, item.bodyAssets, BASE);
      const mdText = flat(md);

      // Headings: the same levels, in order.
      const htmlLevels = [
        ...html.querySelectorAll("h1, h2, h3, h4, h5, h6"),
      ].map((h) => Number(h.tagName[1]));
      // A heading may sit in a list item ("- ### Tx"), as CommonMark allows.
      const mdLevels = md.split("\n").flatMap((line) => {
        const match = /^\s*(?:(?:[-*]|\d+\.) )?(#{1,6}) /.exec(line);
        return match ? [match[1]!.length] : [];
      });
      expect(mdLevels).toEqual(htmlLevels);

      // Links: every href the renderer writes.
      for (const a of html.querySelectorAll("a"))
        expect(md).toContain(`](${a.getAttribute("href")!.trim()})`);
      // Embeds: each iframe's link is kept as a plain link.
      expect((md.match(/\]\(/g) ?? []).length).toBeGreaterThanOrEqual(
        html.querySelectorAll("a, iframe").length,
      );
      // Images: every source.
      for (const img of html.querySelectorAll("img"))
        expect(md).toContain(`(${img.getAttribute("src")})`);

      // Text: every visible text run, whitespace-normalized.
      const walker = document.createTreeWalker(html, NodeFilter.SHOW_TEXT);
      for (
        let current = walker.nextNode();
        current;
        current = walker.nextNode()
      ) {
        const value = flat(current.nodeValue ?? "");
        if (value) expect(mdText).toContain(value);
      }
    },
  );

  it("leads a whole post with its title, standfirst and category, date and author", async () => {
    const item = await post("announcements/mvfw23-recap");
    const md = blogArticleToMarkdown(item, BASE);
    expect(
      md.startsWith(
        `# ${item.title}\n\n${item.description}\n\n_${item.category.title} · ${item.publishedDate.slice(0, 10)} · by ${item.author.title}_`,
      ),
    ).toBe(true);
  });
});
