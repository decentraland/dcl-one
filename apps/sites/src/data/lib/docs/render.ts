// Committed docs Markdown -> the page body HTML the docs layout shows: GitHub alerts back
// into GitBook hints, GitHub-style heading ids, collapsible <details> and lazy images
// with their intrinsic dimensions.
import { Marked, type Tokens } from "marked";

import { pageIconSvg } from "./icons";
import { addImageDimensions, type DocsMediaEntry } from "./images";
import type { DocsTocEntry } from "./types";

const HINT_KIND: Record<string, string> = { NOTE: "info", WARNING: "warning", CAUTION: "danger", TIP: "success", IMPORTANT: "info" };
const HINT_LABEL: Record<string, string> = { warning: "Warning", danger: "Danger", info: "Info", success: "Success" };
const HINT_ICON: Record<string, string> = {
  warning: '<circle cx="12" cy="12" r="9"/><path d="M12 7v6M12 17h.01"/>',
  danger: '<circle cx="12" cy="12" r="9"/><path d="M12 7v6M12 17h.01"/>',
  info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7h.01"/>',
  success: '<circle cx="12" cy="12" r="9"/><path d="m8 12 2.5 2.5L16 9"/>',
};

export function escapeHtml(s: unknown): string {
  return String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c] as string);
}

function decodeEntities(s: string): string {
  return s.replace(/&(?:amp|lt|gt|quot|#39);/g, (e) => ({ "&amp;": "&", "&lt;": "<", "&gt;": ">", "&quot;": '"', "&#39;": "'" })[e] as string);
}

function plainText(html: string): string {
  return decodeEntities(html.replace(/<[^>]*>/g, "")).trim();
}

/** GitHub's (and pandoc's gfm) heading identifier: the ids the upstream anchors point at. */
export function headingSlugger(): (text: string) => string {
  const seen = new Map<string, number>();
  return (text) => {
    const base = text.toLowerCase().trim().replace(/[^\p{L}\p{M}\p{N}\s_-]/gu, "").replace(/\s/g, "-") || "section";
    const count = seen.get(base) ?? 0;
    seen.set(base, count + 1);
    return count ? `${base}-${count}` : base;
  };
}

function hint(kind: string, body: string): string {
  const icon = HINT_ICON[kind] ?? HINT_ICON.info;
  return `<div class="hint hint-${escapeHtml(kind)}" role="note" aria-label="${HINT_LABEL[kind] ?? "Note"}"><div class="hint-icon"><svg viewBox="0 0 24 24" aria-hidden="true">${icon}</svg></div><div class="hint-content">${body}</div></div>\n`;
}

function markdownToHtml(md: string): string {
  const slug = headingSlugger();
  const marked = new Marked({ gfm: true, async: false });
  marked.use({
    renderer: {
      heading(this: { parser: { parseInline(tokens: Tokens.Heading["tokens"]): string } }, { tokens, depth }: Tokens.Heading) {
        const inner = this.parser.parseInline(tokens);
        return `<h${depth} id="${escapeHtml(slug(plainText(inner)))}">${inner}</h${depth}>\n`;
      },
      blockquote(this: { parser: { parse(tokens: Tokens.Blockquote["tokens"]): string } }, { tokens, text }: Tokens.Blockquote) {
        const alert = text.match(/^\[!(\w+)\]/);
        const body = this.parser.parse(tokens);
        if (!alert || !HINT_KIND[alert[1].toUpperCase()]) return `<blockquote>\n${body}</blockquote>\n`;
        const content = body.replace(/^<p>\[!\w+\]\s*/, "<p>").replace(/^<p><\/p>\n?/, "").trim();
        return hint(HINT_KIND[alert[1].toUpperCase()], content);
      },
    },
  });
  return marked.parse(md) as string;
}

function enhance(html: string): string {
  html = html.replace(/<(pre|table)(\b[^>]*)>/g, '<$1$2 tabindex="0">');
  html = html.replace(/<img\b(?![^>]*\bloading=)([^>]*)>/g, '<img loading="lazy" decoding="async"$1>');
  return html.replace(/<details>\s*<summary>([\s\S]*?)<\/summary>([\s\S]*?)<\/details>/g, (_match, label: string, body: string) => {
    const id = plainText(label).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
    return `<details id="${escapeHtml(id)}"><summary><svg class="details-chevron" viewBox="0 0 8 12" aria-hidden="true"><path d="m1 1 5 5-5 5"/></svg>${label}<a class="details-anchor" href="#${escapeHtml(id)}" aria-label="Direct link to heading">#</a></summary><div class="details-body">${body.trim()}</div></details>`;
  });
}

export function tocOf(html: string): DocsTocEntry[] {
  const out: DocsTocEntry[] = [];
  for (const m of html.matchAll(/<h([23])\b[^>]*\bid="([^"]+)"[^>]*>([\s\S]*?)<\/h\1>/g)) {
    out.push({ depth: Number(m[1]) as 2 | 3, id: decodeEntities(m[2]), text: plainText(m[3]) });
  }
  return out;
}

export interface RenderInput {
  markdown: string;
  description: string | null;
  icon: string | null;
  images?: (src: string) => DocsMediaEntry | undefined;
}

/** The page body: the stored agent Markdown minus its description quote, which becomes the lead. */
export function renderDocsPage({ markdown, description, icon, images }: RenderInput): { html: string; toc: DocsTocEntry[] } {
  let md = markdown;
  if (description && md.startsWith(`> ${description}\n\n`)) md = md.slice(description.length + 4);
  let html = enhance(markdownToHtml(md));
  if (images) html = addImageDimensions(html, images);
  const headingIcon = pageIconSvg(icon);
  if (headingIcon) html = html.replace(/(<h1\b[^>]*>)/, `$1${headingIcon}`);
  if (description && !html.includes(escapeHtml(description))) html = html.replace("</h1>", `</h1><p class="lead">${escapeHtml(description)}</p>`);
  return { html, toc: tocOf(html) };
}
