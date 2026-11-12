import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import { legalDocToMarkdown } from "../lib/agent/markdown";
import { toAgentLegalDoc } from "./agent";
import { LEGAL_DOCS, type LegalKey } from "./index";

const FIXTURES = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  "fixtures",
);

type Node = {
  text?: string;
  anchor?: string;
  tag?: string;
  href?: string;
  children?: Node[];
};

const markdownOf = (key: LegalKey) =>
  legalDocToMarkdown(toAgentLegalDoc(LEGAL_DOCS[key]));
const fixture = (slug: string) =>
  JSON.parse(fs.readFileSync(path.join(FIXTURES, `${slug}.json`), "utf8")) as {
    canonical: Node[];
  };

const textOf = (node: Node): string =>
  node.text ?? (node.children ?? []).map(textOf).join("");

/** The document's headings and text blocks in order, from the frozen render: no sidebar or contents. */
function blocks(canonical: Node[]): string[] {
  const out: string[] = [];
  let tocSkipped = false;
  for (const node of canonical) {
    if (node.tag === "a" && !out.length && node.href?.startsWith("/")) continue; // the sidebar
    if (node.tag === "ul" && !tocSkipped) {
      tocSkipped = true; // the table of contents, right after the h1
      continue;
    }
    if (node.tag === "ul") out.push(...(node.children ?? []).map(textOf));
    else if (node.tag) out.push(textOf(node));
  }
  return out.map((t) => t.replace(/\s+/g, " ").trim()).filter(Boolean);
}

// What the converter writes, read back as plain text: link and emphasis syntax removed.
const plain = (md: string) =>
  md
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\*\*([^*]*)\*\*/g, "$1")
    .replace(/^#{1,6} |^- /gm, "")
    .replace(/\s+/g, " ");

describe("agent markdown of the legal documents", () => {
  describe.each(["brand", "content", "ethics"] as const)(
    "when the document has a markdown fixture (%s)",
    (key) => {
      it("should be byte-identical to it", () => {
        const baseline = fs.readFileSync(
          path.join(FIXTURES, "markdown", `${key}.md`),
          "utf8",
        );

        expect(markdownOf(key)).toBe(baseline);
      });
    },
  );

  describe.each([
    ["rewards", "rewards-terms"],
    ["credits", "credits-terms"],
    ["security", "security"],
    ["privacy", "privacy"],
    ["referral", "referral-terms"],
    ["terms", "terms"],
  ] as const)("when the document has new content (%s)", (key, slug) => {
    it("should carry every heading and text block of the frozen render, in order", () => {
      const text = plain(markdownOf(key));
      let from = 0;
      for (const block of blocks(fixture(slug).canonical)) {
        const at = text.indexOf(block, from);
        expect(
          at,
          `missing or out of order: ${block.slice(0, 80)}`,
        ).toBeGreaterThanOrEqual(0);
        from = at + block.length;
      }
    });
  });

  describe("when a document links to another page", () => {
    it("should keep every href", () => {
      const terms = markdownOf("terms");
      const brand = markdownOf("brand");

      expect(terms).toContain("](https://dao.decentraland.org)");
      expect(terms).toContain("](https://decentraland.org/privacy)");
      expect(terms).toContain("](https://market.decentraland.org)");
      expect(terms).toContain("](https://github.com/ethereum/EIPs/issues/20)");
      expect(terms).toContain("](https://decentraland.org/brand)");
      expect(brand).toContain("[Terms of Use](https://decentraland.org/terms)");
      expect(brand).toContain("[Content Policy](https://decentraland.org/content)");
    });
  });

  describe("when a link opens in a new tab", () => {
    it("should keep its href", () => {
      const link = {
        type: "link" as const,
        href: "https://example.com/",
        target: "_blank",
        rel: "noopener noreferrer",
        children: ["example"],
      };
      const doc = {
        ...LEGAL_DOCS.brand,
        sections: [
          {
            id: "x",
            blocks: [
              { type: "h2" as const, content: ["X"] },
              { type: "p" as const, content: [link] },
            ],
          },
        ],
      };

      expect(legalDocToMarkdown(toAgentLegalDoc(doc))).toContain(
        "[example](https://example.com/)",
      );
    });
  });

  describe("when /security is read as markdown", () => {
    it("should keep the report link with its mailto href", () => {
      expect(markdownOf("security")).toContain(
        "[Report a security issue](mailto:security@decentraland.org)",
      );
    });
  });

  describe("when an intro has several paragraphs", () => {
    it("should keep them as separate paragraphs", () => {
      const intro = LEGAL_DOCS.credits.intro.map((b) =>
        b.type === "p" ? b.content.join("") : "",
      );
      const md = markdownOf("credits");

      expect(intro).toHaveLength(3);
      expect(md).toContain(`${intro[0]}\n\n${intro[1]}\n\n${intro[2]}`);
    });
  });

  describe("which documents write their intro as blocks", () => {
    it("should be only security, credits and terms", () => {
      const blocks = (Object.keys(LEGAL_DOCS) as LegalKey[]).filter((key) =>
        Array.isArray(toAgentLegalDoc(LEGAL_DOCS[key]).intro),
      );

      expect(blocks.sort()).toEqual(["credits", "security", "terms"]);
    });
  });

  describe("when a link opens in place", () => {
    it("should keep its href", () => {
      expect(markdownOf("privacy")).toContain(
        "](mailto:privacy@decentraland.org)",
      );
    });
  });

  describe("when a section does not start with its heading", () => {
    it("should fail instead of dropping the heading", () => {
      const doc = {
        ...LEGAL_DOCS.content,
        sections: [
          { id: "x", blocks: [{ type: "p" as const, content: ["text"] }] },
        ],
      };

      expect(() => toAgentLegalDoc(doc)).toThrow("does not start with its h2");
    });
  });
});
