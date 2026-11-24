import type { EntryContext } from "react-router";
import { beforeEach, describe, expect, it } from "vitest";

import { renderAgentMarkdown } from "./dispatch.server";

const context = (key: "blogIndex" | "blogPost", data: unknown, statusCode: number) =>
  ({
    staticHandlerContext: {
      matches: [{ route: { id: "routes/blog", handle: { agentMarkdown: key } } }],
      loaderData: { "routes/blog": data },
      statusCode,
    },
  }) as unknown as EntryContext;

describe("blog agent markdown", () => {
  let card: {
    slug: string;
    title: string;
    description: string;
    publishedAt: string;
    readMinutes: number;
    category: { slug: string; title: string };
    author: { slug: string; name: string; image: string };
    image: null;
    hue: number;
  };

  beforeEach(() => {
    card = {
      slug: "a-post",
      title: "A Post",
      description: "",
      publishedAt: "2026-07-01",
      readMinutes: 1,
      category: { slug: "platform", title: "Platform" },
      author: { slug: "", name: "Decentraland", image: "" },
      image: null,
      hue: 1,
    };
  });

  describe("when the list loaded", () => {
    it("should serialize the list with its status", () => {
      expect(renderAgentMarkdown(context("blogIndex", { state: "ok", posts: [card] }, 200))).toMatchObject({
        status: 200,
        md: expect.stringContaining("1 post."),
      });
    });
  });

  describe("when the post loaded", () => {
    it("should serialize the post with its status", () => {
      expect(renderAgentMarkdown(context("blogPost", { state: "ok", post: { ...card, body: null, assets: {} } }, 200))).toMatchObject({
        status: 200,
        md: expect.stringContaining("# A Post"),
      });
    });
  });

  describe.each([
    ["a 404", { state: "not-found", kind: "category" }, 404],
    ["a 503", { state: "unavailable" }, 503],
    ["no data", undefined, 200],
  ])("when the loader answered %s", (_name, data, status) => {
    it("should leave the list to the HTML page instead of answering 0 posts", () => {
      expect(renderAgentMarkdown(context("blogIndex", data, status))).toBeNull();
    });

    it("should leave the post to the HTML page", () => {
      expect(renderAgentMarkdown(context("blogPost", data, status))).toBeNull();
    });
  });
});
