import type { EntryContext } from "react-router";
import { describe, expect, it } from "vitest";

import { renderAgentMarkdown } from "./dispatch.server";

const context = (key: "blogIndex" | "blogPost", data: unknown, statusCode: number) =>
  ({
    staticHandlerContext: {
      matches: [{ route: { id: "routes/blog", handle: { agentMarkdown: key } } }],
      loaderData: { "routes/blog": data },
      statusCode,
    },
  }) as unknown as EntryContext;

const card = {
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

describe("blog agent markdown", () => {
  it("serializes a loaded list and post with their status", () => {
    expect(renderAgentMarkdown(context("blogIndex", { state: "ok", posts: [card] }, 200))).toMatchObject({
      status: 200,
      md: expect.stringContaining("1 post."),
    });
    expect(renderAgentMarkdown(context("blogPost", { state: "ok", post: { ...card, body: null, assets: {} } }, 200))).toMatchObject({
      status: 200,
      md: expect.stringContaining("# A Post"),
    });
  });

  it.each([
    ["a 404", { state: "not-found", kind: "category" }, 404],
    ["a 503", { state: "unavailable" }, 503],
    ["no data", undefined, 200],
  ])("leaves %s to the HTML page instead of answering 0 posts", (_name, data, status) => {
    expect(renderAgentMarkdown(context("blogIndex", data, status))).toBeNull();
    expect(renderAgentMarkdown(context("blogPost", data, status))).toBeNull();
  });
});
