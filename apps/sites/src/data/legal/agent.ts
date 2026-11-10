import type { AgentLegalDoc } from "../lib/agent/markdown";
import type { LegalBlock, LegalDocument, LegalInline, LegalKey } from "./types";

// The input of the established legal markdown converter (legalDocToMarkdown), built from a legal
// document. The converter keeps the href only of an element of type "a". The previous dcl-one content
// wrapped some links in a component (ui3 `Ext`), so those reached the converter as text only. That
// measured loss is kept for exactly those links, by document and href, so agent markdown keeps its
// contract here; every other link keeps its href. A converter fix is a separate decision.
const EXT_WRAPPED: Partial<Record<LegalKey, ReadonlySet<string>>> = {
  brand: new Set([
    "https://decentraland.org/terms/",
    "https://decentraland.org/content/",
    "https://decentraland.org/press/",
  ]),
  terms: new Set([
    "https://dao.decentraland.org",
    "https://decentraland.zone/bevy-web",
    "https://decentraland.org",
    "https://decentraland.org/privacy",
    "https://decentraland.org/content",
  ]),
};

type Element = { type: string; props: { href?: string; children: unknown } };

function inline(
  node: LegalInline,
  wrapped: ReadonlySet<string> | undefined,
): string | Element {
  if (typeof node === "string") return node;
  const children = node.children.map((child) => inline(child, wrapped));
  if (node.type === "link") {
    return wrapped?.has(node.href)
      ? { type: "external", props: { children } }
      : { type: "a", props: { href: node.href, children } };
  }
  return { type: node.type, props: { children } };
}

const text = (nodes: LegalInline[]): string =>
  nodes.map((n) => (typeof n === "string" ? n : text(n.children))).join("");

type AgentBlock = AgentLegalDoc["sections"][number]["body"][number];

function block(
  b: LegalBlock,
  wrapped: ReadonlySet<string> | undefined,
): AgentBlock {
  const run = (nodes: LegalInline[]) =>
    nodes.map((node) => inline(node, wrapped));
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
  const wrapped = EXT_WRAPPED[doc.key];
  const [only, ...more] = doc.intro;
  const plain =
    !doc.cta &&
    !more.length &&
    only?.type === "p" &&
    only.content.every((node) => typeof node === "string");
  const intro: AgentLegalDoc["intro"] = plain
    ? text(only.content)
    : [
        ...doc.intro.map((b) => block(b, wrapped)),
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
      body: rest.map((b) => block(b, wrapped)),
    };
  });
  return {
    title: doc.title,
    ...(doc.intro.length || doc.cta ? { intro } : {}),
    sections,
  };
}
