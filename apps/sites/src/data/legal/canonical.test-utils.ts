// The semantic tree the legal fixtures were captured as (apps/sites/src/data/legal/fixtures), from a
// rendered DOM. The same algorithm captured the pinned source and production; it is the comparison
// both the tests and the browser checks run against our render.
//
// - Region: the nearest ancestor of the h1 that holds the documents sidebar (its /brand link).
// - Each element keeps tag, id, href, target, rel, aria-current and lang, and its children in order;
//   svg, script, style, noscript and template are skipped (the sidebar icons are decorative).
// - Text: only U+0020, U+0009, U+000A and U+000D collapse to one U+0020, as a line lays out: one
//   space across inline boundaries (kept between inline elements), none at a block's start or end.
//   Every other code point (NBSP, curly quotes, dashes) stays as it is.
// - Canonical: generic containers (div, span, section, aside, nav, article, main, header) are
//   flattened; an id or lang on one becomes an {anchor} or {lang} marker in its place.

export type CanonicalNode =
  | { text: string }
  | { anchor: string }
  | { lang: string }
  | {
      tag: string;
      id?: string;
      href?: string;
      target?: string;
      rel?: string;
      "aria-current"?: string;
      lang?: string;
      src?: string;
      alt?: string;
      children: CanonicalNode[];
    };

type RawNode =
  | { text: string }
  | { tag: string; attrs: Record<string, string>; children: RawNode[] };

const GENERIC = new Set([
  "div",
  "span",
  "section",
  "aside",
  "nav",
  "article",
  "main",
  "header",
]);
const SKIP = new Set(["svg", "script", "style", "noscript", "template"]);
const ATTRS = ["id", "href", "target", "rel", "aria-current", "lang"] as const;
type Attr = (typeof ATTRS)[number];
const WS = /[ \t\n\r]+/g;

export function legalRegion(root: ParentNode): Element | null {
  for (const h1 of root.querySelectorAll("h1")) {
    let node = h1.parentElement;
    while (node && !node.querySelector('a[href="/brand"]'))
      node = node.parentElement;
    if (node) return node;
  }
  return null;
}

// Elements that flow inside a line; every other element starts a block.
const INLINE = new Set([
  "a",
  "strong",
  "em",
  "b",
  "i",
  "u",
  "span",
  "code",
  "small",
  "sub",
  "sup",
  "br",
]);

function raw(el: Element, attrNames: readonly Attr[] = ATTRS): RawNode {
  const attrs: Record<string, string> = {};
  for (const name of attrNames) {
    const value = el.getAttribute(name);
    if (value !== null) attrs[name] = value;
  }
  const children: RawNode[] = [];
  for (const child of el.childNodes) {
    let node: RawNode | null = null;
    if (child.nodeType === 3)
      node = { text: (child.nodeValue ?? "").replace(WS, " ") };
    else if (
      child.nodeType === 1 &&
      !SKIP.has((child as Element).tagName.toLowerCase())
    )
      node = raw(child as Element, attrNames);
    if (!node) continue;
    const last = children[children.length - 1];
    if ("text" in node && last && "text" in last) last.text += node.text;
    else children.push(node);
  }
  return { tag: el.tagName.toLowerCase(), attrs, children };
}

const isInline = (node: RawNode) => "text" in node || INLINE.has(node.tag);

/** The text leaves of an inline run, in order, through inline elements. */
function leaves(
  run: RawNode[],
  out: { text: string }[] = [],
): { text: string }[] {
  for (const node of run) {
    if ("text" in node) out.push(node);
    else if (INLINE.has(node.tag)) leaves(node.children, out);
  }
  return out;
}

/**
 * Collapses spaces as a browser lays out a line: one space across inline boundaries, none at the
 * start or end of a block (or next to a block child). Spaces between inline elements are kept.
 */
function trimBlocks(node: RawNode): RawNode {
  if ("text" in node) return node;
  const children = node.children.map(trimBlocks);
  if (!INLINE.has(node.tag)) {
    let run: RawNode[] = [];
    const flush = () => {
      const texts = leaves(run);
      let spaceBefore = true;
      for (const leaf of texts) {
        if (spaceBefore) leaf.text = leaf.text.replace(/^ /, "");
        if (leaf.text) spaceBefore = leaf.text.endsWith(" ");
      }
      for (let i = texts.length - 1; i >= 0; i -= 1) {
        const leaf = texts[i]!;
        leaf.text = leaf.text.replace(/ $/, "");
        if (leaf.text) break;
      }
      run = [];
    };
    for (const child of children) {
      if (isInline(child)) run.push(child);
      else flush();
    }
    flush();
  }
  const prune = (list: RawNode[]): RawNode[] =>
    list
      .filter((c) => !("text" in c) || c.text !== "")
      .map((c) => ("text" in c ? c : { ...c, children: prune(c.children) }));
  return { ...node, children: prune(children) };
}

function canon(
  node: RawNode,
  attrNames: readonly Attr[] = ATTRS,
): CanonicalNode[] {
  if ("text" in node) return [{ text: node.text }];
  const children = node.children.flatMap((child) => canon(child, attrNames));
  if (GENERIC.has(node.tag)) {
    const marks: CanonicalNode[] = [];
    if (node.attrs.id !== undefined) marks.push({ anchor: node.attrs.id });
    if (node.attrs.lang !== undefined) marks.push({ lang: node.attrs.lang });
    return [...marks, ...children];
  }
  const out: CanonicalNode = { tag: node.tag, children };
  for (const name of attrNames)
    if (node.attrs[name] !== undefined) out[name] = node.attrs[name];
  return [out];
}

function mergeText(list: CanonicalNode[]): CanonicalNode[] {
  const out: CanonicalNode[] = [];
  for (const node of list) {
    if ("children" in node) node.children = mergeText(node.children);
    const last = out[out.length - 1];
    if ("text" in node && last && "text" in last) last.text += node.text;
    else out.push(node);
  }
  return out;
}

/** The canonical tree of a rendered legal region. */
export function canonicalTree(
  region: Element,
  attrNames: readonly Attr[] = ATTRS,
): CanonicalNode[] {
  return mergeText(canon(trimBlocks(raw(region, attrNames)), attrNames));
}

/** The canonical tree of the legal region under `root`. */
export function canonicalLegalTree(root: ParentNode): CanonicalNode[] {
  const region = legalRegion(root);
  if (!region)
    throw new Error("no legal region (an h1 next to the /brand sidebar link)");
  return mergeText(canon(trimBlocks(raw(region))));
}

/**
 * Our render marks the active sidebar entry with aria-current="page", which source does not (its
 * active entry is a style only). Removes that one mark, and fails if any other node carries one, so
 * the rest of the tree compares exactly.
 */
export function withoutCurrentMarks(
  tree: CanonicalNode[],
  activeHref: string,
): CanonicalNode[] {
  let found = 0;
  const strip = (node: CanonicalNode): CanonicalNode => {
    if (!("children" in node)) return node;
    const { "aria-current": mark, ...rest } = node;
    if (mark === undefined)
      return { ...rest, children: node.children.map(strip) };
    if (node.tag !== "a" || node.href !== activeHref || mark !== "page")
      throw new Error(
        `unexpected aria-current="${mark}" on <${node.tag}> ${node.href ?? ""}`,
      );
    found += 1;
    return { ...rest, children: node.children.map(strip) };
  };
  const out = tree.map(strip);
  if (found !== 1)
    throw new Error(
      `expected aria-current="page" once, on ${activeHref}; found ${found}`,
    );
  return out;
}

/**
 * In a locale other than English our region carries lang="en" (source has no such mark). Removes
 * that marker, which must lead the region, and fails on a lang anywhere else.
 */
export function withoutRegionLang(
  tree: CanonicalNode[],
  lang: string,
): CanonicalNode[] {
  const [first, ...rest] = tree;
  if (
    !first ||
    !("lang" in first) ||
    "children" in first ||
    first.lang !== lang
  )
    throw new Error(`expected the legal region to carry lang="${lang}"`);
  const check = (node: CanonicalNode) => {
    if ("lang" in node)
      throw new Error(`unexpected lang="${node.lang}" inside the legal region`);
    if ("children" in node) node.children.forEach(check);
  };
  rest.forEach(check);
  return rest;
}
