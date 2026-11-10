// A blog post for agents (Accept: text/markdown, ?format=md): its header, then its body serialized to
// markdown with the same node rules as the HTML renderer (library/ui/apps/sites/blog/BlogRichText.tsx,
// the R4-4 node set). Headings keep their rendered level (a heading-6 renders as h5, so ##### here);
// embeds become plain links to their URL; a link shows its first text only; an unsafe link is plain
// text (the renderer's own allowlist, library/ui blog/links.ts); a node outside the set contributes
// its text, as the renderer renders its children.
import { isSafeHyperlinkUri } from "@dcl/one-ui/apps/sites/blog/links";

import type { BlogAsset, BlogPost, RichTextNode } from "../blog/types";

type Context = {
  assets: Readonly<Record<string, BlogAsset>>;
  blogBaseUrl: string;
};

const HEADINGS: Record<string, number> = {
  "heading-1": 1,
  "heading-2": 2,
  "heading-3": 3,
  "heading-4": 4,
  "heading-5": 5,
  // As rendered: the source's Heading6 is styled(Heading5).
  "heading-6": 5,
};

const MARKS: Record<string, (text: string) => string> = {
  bold: (text) => `**${text}**`,
  italic: (text) => `_${text}_`,
  code: (text) => `\`${text}\``,
  // No markdown for underline: the text stays.
  underline: (text) => text,
};

const flat = (text: string) => text.replace(/\s+/g, " ");

function inline(node: RichTextNode, ctx: Context): string {
  if (node.nodeType === "text") {
    const value = flat(node.value ?? "");
    if (!value.trim()) return value;
    // Marks wrap the text without its edge spaces, which markdown would not parse inside a delimiter.
    const lead = value.match(/^\s*/)![0];
    const tail = value.match(/\s*$/)![0];
    const marked = (node.marks ?? []).reduce(
      (text, mark) => MARKS[mark.type]?.(text) ?? text,
      value.trim(),
    );
    return `${lead}${marked}${tail}`;
  }
  if (node.nodeType === "hyperlink") {
    const uri = typeof node.data?.uri === "string" ? node.data.uri : "";
    const text = flat(node.content?.[0]?.value ?? "");
    if (ctx.blogBaseUrl.length > 0 && uri.startsWith(ctx.blogBaseUrl)) {
      return `[${text}](${new URL(uri, "https://blog.invalid").pathname})`;
    }
    if (!isSafeHyperlinkUri(uri)) return text;
    return `[${text}](${uri.trim()})`;
  }
  return (node.content ?? []).map((child) => inline(child, ctx)).join("");
}

function image(node: RichTextNode, ctx: Context): string | null {
  const target = node.data?.target as
    { sys?: { id?: string }; fields?: { file?: { url?: string } } } | undefined;
  const asset = target?.sys?.id ? ctx.assets[target.sys.id] : undefined;
  if (asset) return `![${asset.title ?? ""}](${asset.url})`;
  const url = target?.fields?.file?.url;
  return url ? `![](${url.startsWith("//") ? `https:${url}` : url})` : null;
}

function list(node: RichTextNode, ctx: Context, depth: number): string {
  const ordered = node.nodeType === "ordered-list";
  const pad = "  ".repeat(depth);
  return (node.content ?? [])
    .map((item, index) => {
      const marker = ordered ? `${index + 1}.` : "-";
      // The item's first block follows its marker; each later block (a paragraph, a heading, a nested
      // list) starts its own line inside the item, so markdown still reads it as that block.
      const parts = (item.content ?? [])
        .map((child) =>
          child.nodeType === "unordered-list" ||
          child.nodeType === "ordered-list"
            ? list(child, ctx, depth + 1)
            : (block(child, ctx, depth + 1) ?? ""),
        )
        .filter((part) => part.trim().length > 0);
      const [first = "", ...rest] = parts;
      const inner = `${pad}  `;
      const following = rest.map((part) =>
        part.startsWith(inner) ? part : `${inner}${part}`,
      );
      return [`${pad}${marker} ${first.trimStart()}`, ...following].join("\n");
    })
    .join("\n");
}

function table(node: RichTextNode, ctx: Context): string {
  const rows = (node.content ?? []).map((row) =>
    (row.content ?? []).map((cell) =>
      (cell.content ?? [])
        .map((child) => inline(child, ctx))
        .join(" ")
        .trim()
        .replace(/\|/g, "\\|"),
    ),
  );
  if (rows.length === 0) return "";
  const width = Math.max(...rows.map((row) => row.length));
  const line = (cells: string[]) =>
    `| ${Array.from({ length: width }, (_, i) => cells[i] ?? "").join(" | ")} |`;
  return [
    line(rows[0]!),
    line(Array.from({ length: width }, () => "---")),
    ...rows.slice(1).map(line),
  ].join("\n");
}

function block(node: RichTextNode, ctx: Context, depth = 0): string | null {
  const level = HEADINGS[node.nodeType];
  if (level) return `${"#".repeat(level)} ${inline(node, ctx).trim()}`;
  switch (node.nodeType) {
    case "paragraph":
      return inline(node, ctx).trim() || null;
    case "unordered-list":
    case "ordered-list":
      return list(node, ctx, depth);
    case "blockquote":
      return blocks(node.content ?? [], ctx)
        .split("\n")
        .map((lineText) => `> ${lineText}`.trimEnd())
        .join("\n");
    case "hr":
      return "---";
    case "table":
      return table(node, ctx);
    case "embedded-asset-block":
      return image(node, ctx);
    default: {
      // Outside the set: its text, as the renderer renders its children; nothing when it has none.
      const text = inline(node, ctx).trim();
      return text || null;
    }
  }
}

function blocks(nodes: RichTextNode[], ctx: Context): string {
  return nodes
    .map((node) => block(node, ctx))
    .filter((text): text is string => !!text && text.trim().length > 0)
    .join("\n\n");
}

/** The body, serialized. `blogBaseUrl` is the configured public /blog, as the HTML renderer gets it. */
export function blogBodyToMarkdown(
  body: RichTextNode | null,
  assets: Readonly<Record<string, BlogAsset>>,
  blogBaseUrl: string,
): string {
  if (!body?.content) return "";
  return blocks(body.content, { assets, blogBaseUrl });
}

/** A whole post: title, standfirst, category · date · author, then the body. */
export function blogArticleToMarkdown(
  post: BlogPost,
  blogBaseUrl: string,
): string {
  const meta = [
    post.category.title,
    post.publishedDate.slice(0, 10),
    post.author.title ? `by ${post.author.title}` : "",
  ]
    .filter((part) => part.trim().length > 0)
    .join(" \u{B7} ");
  return [
    `# ${flat(post.title).trim()}`,
    post.description ? flat(post.description).trim() : "",
    meta ? `_${meta}_` : "",
    blogBodyToMarkdown(post.body, post.bodyAssets, blogBaseUrl),
  ]
    .filter((part) => part.length > 0)
    .join("\n\n");
}
