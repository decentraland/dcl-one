// @vitest-environment jsdom
import { isDeepStrictEqual } from "node:util";
import { describe, expect, it } from "vitest";

import { canonicalLegalTree } from "./legal-canonical";

/** The canonical tree of a minimal legal region whose document body is `body`. */
function tree(body: string) {
  const root = document.createElement("div");
  // Test markup only; nothing here comes from outside the test.
  root.innerHTML = `<div><div><a href="/brand">Logo and Name</a></div><div><h1>Title</h1>${body}</div></div>`;
  return canonicalLegalTree(root);
}

const same = (a: string, b: string) => isDeepStrictEqual(tree(a), tree(b));

describe("the legal canonical tree", () => {
  describe("when a space sits at the start of an inline element", () => {
    it("should keep it, so joined words do not compare equal", () => {
      expect(
        same(
          "<p>Hello<strong> world</strong></p>",
          "<p>Hello<strong>world</strong></p>",
        ),
      ).toBe(false);
    });
  });

  describe("when a space is the only text between two inline elements", () => {
    it("should keep it, so joined words do not compare equal", () => {
      expect(
        same(
          '<p><strong>Hello</strong> <a href="/x">world</a></p>',
          '<p><strong>Hello</strong><a href="/x">world</a></p>',
        ),
      ).toBe(false);
    });
  });

  describe("when a space ends an inline element before more text", () => {
    it("should keep it", () => {
      expect(
        same(
          "<p><strong>Hello </strong>world</p>",
          "<p><strong>Hello</strong>world</p>",
        ),
      ).toBe(false);
    });
  });

  describe("when spaces sit at a block's start or end", () => {
    it("should drop them, as layout does", () => {
      expect(
        same(
          "<p>  Hello <strong>world</strong>\n </p>",
          "<p>Hello <strong>world</strong></p>",
        ),
      ).toBe(true);
    });
  });

  describe("when spaces repeat across an inline boundary", () => {
    it("should count them as one, as layout does", () => {
      expect(
        same(
          "<p>Hello <strong> world</strong></p>",
          "<p>Hello <strong>world</strong></p>",
        ),
      ).toBe(true);
    });
  });

  describe("when a space is a non-breaking space", () => {
    it("should keep it as its own code point", () => {
      expect(
        same(
          "<p>Hello <strong>world</strong></p>",
          "<p>Hello <strong>world</strong></p>",
        ),
      ).toBe(false);
    });
  });
});
