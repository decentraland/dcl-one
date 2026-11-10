// The rich-text node set the blog renders (blog plan R4-4, from the inventory of all 471 published
// posts), and the record of everything outside it (R2-2): every unsupported node is listed with its
// type and position, whether or not it has text, so nothing disappears silently.
import type { RichTextNode } from "./types";

export const SUPPORTED_NODES: ReadonlySet<string> = new Set([
  "document",
  "paragraph",
  "heading-1",
  "heading-2",
  "heading-3",
  "heading-4",
  "heading-5",
  "heading-6",
  "unordered-list",
  "ordered-list",
  "list-item",
  "blockquote",
  "hr",
  "table",
  "table-row",
  "table-header-cell",
  "table-cell",
  "embedded-asset-block",
  "hyperlink",
  "text",
]);

export const SUPPORTED_MARKS: ReadonlySet<string> = new Set([
  "bold",
  "italic",
  "underline",
  "code",
]);

/**
 * Types whose rendering waits on an owner decision: they are recorded like unsupported ones and
 * hold acceptance of the posts that use them (asset-hyperlink: decision O5, 2 posts).
 */
export const HELD_NODES: ReadonlyMap<string, string> = new Map([
  ["asset-hyperlink", "O5"],
]);

export type UnsupportedNode = {
  nodeType: string;
  /** Child indexes from the document root, e.g. [3, 0] is the first child of the fourth block. */
  path: number[];
  /** Whether it has text to render; a node without any is still recorded. */
  hasText: boolean;
  /** The owner decision holding it, when it is a held type rather than an unknown one. */
  heldBy?: string;
};

export type UnsupportedMark = { mark: string; path: number[] };

const hasText = (node: RichTextNode): boolean =>
  (node.nodeType === "text" && !!node.value) ||
  (node.content ?? []).some(hasText);

/** Every node and mark in a body outside the supported set, in document order. */
export function findUnsupported(body: RichTextNode | null | undefined): {
  nodes: UnsupportedNode[];
  marks: UnsupportedMark[];
} {
  const nodes: UnsupportedNode[] = [];
  const marks: UnsupportedMark[] = [];
  const walk = (node: RichTextNode, path: number[]) => {
    if (!SUPPORTED_NODES.has(node.nodeType)) {
      const heldBy = HELD_NODES.get(node.nodeType);
      nodes.push({
        nodeType: node.nodeType,
        path,
        hasText: hasText(node),
        ...(heldBy ? { heldBy } : {}),
      });
    }
    for (const mark of node.marks ?? [])
      if (!SUPPORTED_MARKS.has(mark.type))
        marks.push({ mark: mark.type, path });
    (node.content ?? []).forEach((child, index) =>
      walk(child, [...path, index]),
    );
  };
  if (body) walk(body, []);
  return { nodes, marks };
}

/** One recorded unsupported node or mark, with the post it belongs to (blog plan R2-2). */
export type UnsupportedEntry = (UnsupportedNode | UnsupportedMark) & {
  slug: string;
};

/** A post's unsupported nodes and marks, each with the post slug, for the dev-time report. */
export function unsupportedEntries(
  slug: string,
  body: RichTextNode | null | undefined,
): UnsupportedEntry[] {
  const { nodes, marks } = findUnsupported(body);
  return [...nodes, ...marks].map((entry) => ({ ...entry, slug }));
}
