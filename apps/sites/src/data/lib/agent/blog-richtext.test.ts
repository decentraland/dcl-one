import { describe, expect, it } from "vitest";

import type { RichTextNode } from "../blog/types";
import { blogBodyToMarkdown } from "./blog-richtext";

const BASE = "https://decentraland.org/blog";
const text = (value: string, marks: string[] = []): RichTextNode => ({
  nodeType: "text",
  value,
  marks: marks.map((type) => ({ type })),
});
const node = (
  nodeType: string,
  content: RichTextNode[] = [],
  data: Record<string, unknown> = {},
): RichTextNode => ({ nodeType, content, data });
const doc = (...content: RichTextNode[]) => node("document", content);
const link = (uri: string, label = uri) =>
  node("hyperlink", [text(label)], { uri });
const md = (body: RichTextNode, assets = {}) =>
  blogBodyToMarkdown(body, assets, BASE);

describe("blogBodyToMarkdown", () => {
  it.each([
    ["heading-1", "# H"],
    ["heading-2", "## H"],
    ["heading-3", "### H"],
    ["heading-4", "#### H"],
    ["heading-5", "##### H"],
    // As rendered: h6 renders as h5.
    ["heading-6", "##### H"],
  ])("writes a %s as %j", (type, expected) => {
    expect(md(doc(node(type, [text("H")])))).toBe(expected);
  });

  it("separates blocks with a blank line and marks text, the first mark innermost", () => {
    expect(
      md(
        doc(
          node("paragraph", [
            text("a "),
            text("b", ["bold", "italic"]),
            text(" c"),
          ]),
          node("paragraph", [text("d", ["code"])]),
        ),
      ),
    ).toBe("a _**b**_ c\n\n`d`");
  });

  it("keeps underlined text as plain text and drops an unknown mark", () => {
    expect(
      md(
        doc(
          node("paragraph", [
            text("u", ["underline"]),
            text("x", ["superscript"]),
          ]),
        ),
      ),
    ).toBe("ux");
  });

  it("writes lists, nested lists, a blockquote, a rule and a table", () => {
    const body = doc(
      node("unordered-list", [
        node("list-item", [
          node("paragraph", [text("a")]),
          node("ordered-list", [
            node("list-item", [node("paragraph", [text("b")])]),
          ]),
        ]),
      ]),
      node("blockquote", [node("paragraph", [text("q")])]),
      node("hr"),
      node("table", [
        node("table-row", [
          node("table-header-cell", [node("paragraph", [text("H|1")])]),
        ]),
        node("table-row", [
          node("table-cell", [node("paragraph", [text("C")])]),
        ]),
      ]),
    );
    expect(md(body)).toBe(
      "- a\n  1. b\n\n> q\n\n---\n\n| H\\|1 |\n| --- |\n| C |",
    );
  });

  it("starts each later block of a list item on its own line, so a heading stays a heading", () => {
    const body = doc(
      node("unordered-list", [
        node("list-item", [
          node("paragraph", [text("intro")]),
          node("heading-3", [text("Tx")]),
        ]),
      ]),
    );
    expect(md(body)).toBe("- intro\n  ### Tx");
  });

  describe("links", () => {
    it("writes an external link with its first text only", () => {
      expect(
        md(
          doc(
            node("paragraph", [
              node("hyperlink", [text("one"), text("two")], {
                uri: "https://example.org",
              }),
            ]),
          ),
        ),
      ).toBe("[one](https://example.org)");
    });

    it("writes a blog link as its path, dropping query and hash", () => {
      expect(
        md(
          doc(
            node("paragraph", [
              link(`${BASE}/announcements/post?x=1#top`, "Post"),
            ]),
          ),
        ),
      ).toBe("[Post](/blog/announcements/post)");
    });

    it("writes an embed (a link whose text is its URL) as a plain link to it", () => {
      expect(
        md(doc(node("paragraph", [link("https://youtu.be/abc123")]))),
      ).toBe("[https://youtu.be/abc123](https://youtu.be/abc123)");
    });

    it.each([
      "javascript:alert(1)",
      "javascript://linkedin.com/%0Aalert(1)",
      "data:text/html,x",
      "",
    ])("writes %j as its text only", (uri) => {
      expect(md(doc(node("paragraph", [link(uri, "label")])))).toBe("label");
    });
  });

  it("writes an image with its title as alt, and nothing for an asset it cannot resolve", () => {
    const image = (id: string) =>
      node("embedded-asset-block", [], { target: { sys: { id } } });
    expect(
      md(doc(image("a1"), image("missing")), {
        a1: {
          id: "a1",
          url: "https://cms.example/a.png",
          width: 1,
          height: 1,
          mimeType: "image/png",
          title: "T",
        },
      }),
    ).toBe("![T](https://cms.example/a.png)");
  });

  it("writes a node outside the set as its text, and nothing for a contentless one, as the renderer", () => {
    const asset = node("asset-hyperlink", [text("the asset")], {
      target: { sys: { id: "x" } },
    });
    expect(
      md(
        doc(
          node("paragraph", [text("see "), asset]),
          node("embedded-entry-block", [], { target: { sys: { id: "e" } } }),
        ),
      ),
    ).toBe("see the asset");
  });

  it("writes nothing for a missing body", () => {
    expect(md(null as unknown as RichTextNode)).toBe("");
  });
});
