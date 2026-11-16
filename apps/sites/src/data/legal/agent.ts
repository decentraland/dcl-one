import type { AgentLegalDoc } from "../lib/agent/markdown";
import type { LegalBlock, LegalDocument, LegalInline } from "./types";

// The input of the legal markdown converter (legalDocToMarkdown), built from a legal document.
type Element = { type: string; props: { href?: string; children: unknown } };

function inline(node: LegalInline): string | Element {
  if (typeof node === "string") return node;
  const children = node.children.map(inline);
  if (node.type === "link")
    return { type: "a", props: { href: node.href, children } };
  return { type: node.type, props: { children } };
}

const text = (nodes: readonly LegalInline[]): string =>
  nodes.map((n) => (typeof n === "string" ? n : text(n.children))).join("");

type AgentBlock = AgentLegalDoc["sections"][number]["body"][number];

function block(b: LegalBlock): AgentBlock {
  const run = (nodes: readonly LegalInline[]) => nodes.map(inline);
  if (b.type === "ul") return { type: "ul", items: b.items.map(run) };
  if (b.type === "p") return { content: run(b.content) };
  return { type: b.type, content: run(b.content) };
}

/**
 * The intro: one plain-text paragraph when it is a single paragraph, as before; otherwise (several
 * paragraphs, or a call to action) blocks the converter renders like a section body, so each
 * paragraph stays separate and links keep their href.
 */
export function toAgentLegalDoc(doc: LegalDocument): AgentLegalDoc {
  const [only, ...more] = doc.intro;
  const plain =
    !doc.cta &&
    !more.length &&
    only?.type === "p" &&
    only.content.every((node) => typeof node === "string");
  const intro: AgentLegalDoc["intro"] = plain
    ? text(only.content)
    : [
        ...doc.intro.map(block),
        ...(doc.cta
          ? [
              {
                content: [
                  {
                    type: "a",
                    props: { href: doc.cta.href, children: doc.cta.label },
                  },
                ],
              },
            ]
          : []),
      ];
  const sections = doc.sections.map((section) => {
    const [first, ...rest] = section.blocks;
    if (first?.type !== "h2")
      throw new Error(`legal section ${section.id} does not start with its h2`);
    return {
      id: section.id,
      heading: text(first.content),
      body: rest.map(block),
    };
  });
  return {
    title: doc.title,
    ...(doc.intro.length || doc.cta ? { intro } : {}),
    sections,
  };
}
