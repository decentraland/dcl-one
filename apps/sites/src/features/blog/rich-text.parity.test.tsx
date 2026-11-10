// @vitest-environment jsdom
// The post body render against the source's (blog plan B0 oracles, frozen in
// data/lib/blog/fixtures/oracle-bodies.json): for each of the 11 sampled posts, our BlogRichText over
// the same CMS data must give the same canonical tree, under the scoped normalisations of
// features/test-utils/blog-oracle.ts (R4-4 alt, N3 X posts, O5 debug text).
import "../locale/test-setup";
import { render } from "@testing-library/react";
import { isDeepStrictEqual } from "node:util";
import { describe, expect, it } from "vitest";

import { BlogRichText } from "@dcl/one-ui/apps/sites/blog/BlogRichText";

import { createFixtureBlogSource } from "@/data/lib/blog/fixture-source";
import oracle from "@/data/lib/blog/fixtures/oracle-bodies.json";
import type { BlogPost } from "@/data/lib/blog/types";
import { markTweets, normaliseSourceTree } from "../test-utils/blog-oracle";
import {
  BLOG_ATTRS,
  canonicalTree,
  type CanonicalNode,
} from "../test-utils/legal-canonical";

const BLOG_BASE_URL = "https://decentraland.org/blog";
const bodies = (oracle as { bodies: Record<string, CanonicalNode[]> }).bodies;
const source = createFixtureBlogSource();

async function post(key: string): Promise<BlogPost> {
  const [category, slug] = key.split("/") as [string, string];
  const found = await source.getPost(
    category,
    slug,
    new AbortController().signal,
  );
  if (!found) throw new Error(`fixture post ${key} missing`);
  return found;
}

function ours(body: BlogPost): CanonicalNode[] {
  const { container } = render(
    <BlogRichText
      body={body.body}
      assets={body.bodyAssets}
      blogBaseUrl={BLOG_BASE_URL}
    />,
  );
  markTweets(container);
  return canonicalTree(container, BLOG_ATTRS);
}

const expected = normaliseSourceTree;

describe("the post body against the source render", () => {
  it.each(Object.keys(bodies))("renders %s as the source does", async (key) => {
    const body = await post(key);
    expect(ours(body)).toEqual(expected(bodies[key]!, body));
  });

  it("covers every element the oracle bodies use", () => {
    const tags = new Set<string>();
    const walk = (node: CanonicalNode) => {
      if ("children" in node) {
        tags.add(node.tag);
        node.children.forEach(walk);
      }
    };
    Object.values(bodies).flat().forEach(walk);
    expect([...tags].sort()).toEqual(
      [
        "a",
        "b",
        "blockquote",
        "code",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "hr",
        "i",
        "iframe",
        "img",
        "li",
        "ol",
        "p",
        "table",
        "tbody",
        "td",
        "th",
        "tr",
        "u",
        "ul",
      ].sort(),
    );
  });

  it("fails on a one-character change (control)", async () => {
    const key = Object.keys(bodies)[0]!;
    const body = await post(key);
    const tree = expected(bodies[key]!, body);
    const mutated = structuredClone(tree);
    const firstText = (nodes: CanonicalNode[]): { text: string } | null => {
      for (const node of nodes) {
        if ("text" in node) return node;
        if ("children" in node) {
          const found = firstText(node.children);
          if (found) return found;
        }
      }
      return null;
    };
    firstText(mutated)!.text += "x";
    expect(isDeepStrictEqual(ours(body), mutated)).toBe(false);
  });
});
