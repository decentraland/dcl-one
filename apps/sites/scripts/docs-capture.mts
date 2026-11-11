// The live docs.decentraland.org capture (gen-docs-content.mts --capture): the locale variants
// GitBook serves that the decentraland/docs mirror does not hold. The sitemap index names one
// sitemap-pages.xml per variant; each page is fetched as Markdown (`<page url>.md`). Variants of a
// section list the same pages in the same order, which pairs a page with its translations.
export const LIVE = "https://docs.decentraland.org";
export const CAPTURE_BASE = "/docs";

import { englishMatch } from "../src/data/lib/docs/live.ts";

// The runtime redirect for live docs paths (src/data/lib/docs/content.server.ts) matches the same way.
export { englishMatch };

/** The live section folder -> the variant folder prefix GitBook uses under it. */
const SECTION_PREFIX: Record<string, string> = { player: "player", creator: "content-creator", contributor: "contributor" };
const LANGUAGE: Record<string, string> = { es: "es", pt: "pt-br", ko: "ko", zh: "zh" };

export interface Variant {
  /** The space id, also the last URL segment: `player-pt`, `content-creator-ko`. */
  id: string;
  /** The English space the variant translates: player, creator, contributor. */
  section: string;
  /** The BCP 47 tag the live page declares. */
  language: string;
  /** The language switcher label: ES, PT, KO, ZH. */
  label: string;
  /** The path under /docs and under the live origin: `player/player-pt`. */
  path: string;
  sitemap: string;
}

export interface SitemapEntry { loc: string; lastmod: string | null }

const entities = (apostrophe: RegExp) => (s: string) =>
  s.replace(/&amp;/g, "&").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(apostrophe, "'").trim();
const xmlText = entities(/&apos;/g);
const htmlText = entities(/&#x27;|&#39;/g);

export function parseSitemapIndex(xml: string): string[] {
  return [...xml.matchAll(/<sitemap>[\s\S]*?<loc>([\s\S]*?)<\/loc>[\s\S]*?<\/sitemap>/g)].map((m) => xmlText(m[1]));
}

export function parseSitemap(xml: string): SitemapEntry[] {
  return [...xml.matchAll(/<url>([\s\S]*?)<\/url>/g)].map((m) => ({
    loc: xmlText(m[1].match(/<loc>([\s\S]*?)<\/loc>/)?.[1] ?? ""),
    lastmod: m[1].match(/<lastmod>([\s\S]*?)<\/lastmod>/)?.[1]?.trim() ?? null,
  })).filter((e) => e.loc);
}

/** A per-variant sitemap URL (`<origin>/<section>/<prefix>-<lang>/sitemap-pages.xml`) -> its variant; English spaces -> null. */
export function variantOf(sitemap: string): Variant | null {
  const m = sitemap.match(/^https?:\/\/[^/]+\/([a-z]+)\/([a-z-]+)-([a-z]{2})\/sitemap-pages\.xml$/);
  if (!m || SECTION_PREFIX[m[1]] !== m[2]) return null;
  return { id: `${m[2]}-${m[3]}`, section: m[1], language: LANGUAGE[m[3]] ?? m[3], label: m[3].toUpperCase(), path: `${m[1]}/${m[2]}-${m[3]}`, sitemap };
}

function splitHash(t: string): [string, string] {
  const i = t.indexOf("#");
  return i === -1 ? [t, ""] : [t.slice(0, i), t.slice(i)];
}

/** A live URL or site path inside a captured variant -> its /docs URL (`/docs/player/player-pt/a/b/#x`); anything else -> null. */
export function localUrl(target: string, variants: Variant[]): string | null {
  const [raw, hash] = splitHash(target.trim());
  let p: string;
  if (raw.startsWith(`${LIVE}/`) || raw === LIVE) p = raw.slice(LIVE.length);
  else if (raw.startsWith("/")) p = raw;
  else return null;
  p = p.replace(/\?.*$/, "").replace(/\.md$/i, "").replace(/\/+$/, "").replace(/^\/+/, "");
  const variant = variants.find((v) => p === v.path || p.startsWith(`${v.path}/`));
  return variant ? `${CAPTURE_BASE}/${p}/${hash}` : null;
}

export function pageFile(url: string): string {
  const rel = url.slice(CAPTURE_BASE.length).replace(/^\/+|\/+$/g, "");
  return rel ? `${rel}/index.md` : "index.md";
}

/** The sidebar group titles of a live GitBook page, in order (the server renders every group, not every nested page). */
export function groupTitles(html: string): string[] {
  const start = html.indexOf('data-testid="table-of-contents"');
  if (start === -1) return [];
  const toc = html.slice(start, html.indexOf("</aside>", start));
  return toc.split(/<li class="page-group-item\b/).slice(1).map((part) => {
    const head = part.replace(/<svg[\s\S]*?<\/svg>/g, "").split(/<a\b|<ul\b/)[0];
    return [...head.matchAll(/>([^<>]+)</g)].map((m) => htmlText(m[1])).find(Boolean) ?? "";
  });
}

export interface CaptureNavNode { type?: "section" | "item"; title?: string; url?: string | null; children: CaptureNavNode[] }

const titleize = (slug: string) => slug.replace(/-/g, " ").replace(/^./, (c) => c.toUpperCase());

/**
 * The variant's sidebar from its page URLs in sitemap order. GitBook paths are
 * `<group>/<page>/<child>`: a first segment that is no page is a group, a page under another
 * page is its child. Groups take the captured titles in order when the counts agree.
 */
export function buildNav(root: string, pages: Array<{ url: string; title: string }>, groups: string[]): CaptureNavNode {
  const nav: CaptureNavNode = { children: [] };
  const urls = new Set(pages.map((p) => p.url));
  const items = new Map<string, CaptureNavNode>();
  const sections = new Map<string, CaptureNavNode>();
  const slugs: string[] = [];
  const segments = (url: string) => url.slice(root.length).replace(/\/+$/, "").split("/").filter(Boolean);
  for (const page of pages) {
    const segs = segments(page.url);
    if (segs.length > 1 && !urls.has(`${root}${segs[0]}/`) && !slugs.includes(segs[0])) slugs.push(segs[0]);
  }
  const titled = groups.length === slugs.length;
  for (const page of pages) {
    const node: CaptureNavNode = { type: "item", title: page.title, url: page.url, children: [] };
    items.set(page.url, node);
    const segs = segments(page.url);
    let parent: CaptureNavNode = nav;
    for (let n = segs.length - 1; n >= 1; n--) {
      const prefix = `${root}${segs.slice(0, n).join("/")}/`;
      const hit = items.get(prefix);
      if (hit) { parent = hit; break; }
      if (n === 1 && slugs.includes(segs[0])) {
        let section = sections.get(segs[0]);
        if (!section) {
          section = { type: "section", title: titled ? groups[slugs.indexOf(segs[0])] : titleize(segs[0]), children: [] };
          sections.set(segs[0], section);
          nav.children.push(section);
        }
        parent = section;
      }
    }
    parent.children.push(node);
  }
  return nav;
}

/** A captured page's Markdown -> title, description and body (the export's index banner removed). */
export function splitCaptured(md: string, hasDescription: boolean): { title: string; description: string | null; body: string } {
  let text = md.replace(/\r\n/g, "\n").replace(/^> For the complete documentation index[^\n]*\n+/, "");
  const h1 = text.match(/^#[ \t]+(.+?)[ \t]*$/m);
  const title = h1?.[1]?.trim() ?? "";
  let description: string | null = null;
  if (h1 && hasDescription) {
    const end = (h1.index ?? 0) + h1[0].length;
    const after = text.slice(end).replace(/^\n+/, "");
    const para = after.match(/^([^\n#>|*`<{!-][^\n]*)\n(?:\n|$)/);
    if (para) {
      description = para[1].trim();
      text = text.slice(0, end) + "\n\n" + after.slice(para[0].length).replace(/^\n+/, "");
    }
  }
  return { title, description, body: text.trim() };
}

/** The export escapes `&` inside link and image URLs (`?alt=media\&token=`); the URL needs the plain character. */
export function unescapeUrls(md: string): string {
  return md.replace(/https?:\/\/[^\s)"'<>]+/g, (url) => url.replace(/\\&/g, "&"));
}

/** A captured variant's manifest space: the English space's card, its live path, language and capture record. */
export function variantSpace<S extends { title: string; blurb: string; icon: string }>(variant: Variant, english: S, capturedAt: string) {
  return {
    id: variant.id, title: english.title, blurb: english.blurb, icon: english.icon as S["icon"], path: variant.path, section: variant.section,
    language: variant.language, label: variant.label, source: "capture" as const, capturedAt, sitemap: variant.sitemap,
  };
}

/** Each page's translations by label: variants of a section pair by sitemap position, English by its matched page. */
export function translationsOf(variants: Array<{ variant: Variant; pages: Array<{ url: string; en: string | null }> }>): Map<string, Record<string, string>> {
  const out = new Map<string, Record<string, string>>();
  const add = (url: string, label: string, target: string) => {
    if (url === target) return;
    const entry = out.get(url) ?? {};
    entry[label] ??= target;
    out.set(url, entry);
  };
  for (const v of variants) {
    for (const [i, page] of v.pages.entries()) {
      for (const other of variants) {
        if (other.variant.section === v.variant.section && other.pages[i]) add(page.url, other.variant.label, other.pages[i].url);
      }
      if (page.en) { add(page.url, "EN", page.en); add(page.en, v.variant.label, page.url); }
    }
  }
  return out;
}

/** Run fn over items, at most `limit` at a time, keeping input order in the result. */
export async function mapBounded<T, R>(items: T[], limit: number, fn: (item: T, index: number) => Promise<R>): Promise<R[]> {
  const out = new Array<R>(items.length);
  let next = 0;
  const worker = async () => {
    while (next < items.length) {
      const i = next++;
      out[i] = await fn(items[i], i);
    }
  };
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, worker));
  return out;
}

/** GET with one retry on a 5xx or a network error. */
export async function fetchText(url: string, accept: string): Promise<string> {
  for (let attempt = 0; ; attempt++) {
    try {
      const response = await fetch(url, { headers: { accept }, redirect: "follow", signal: AbortSignal.timeout(60_000) });
      if (response.ok) return await response.text();
      if (response.status < 500 || attempt) throw new Error(`HTTP ${response.status}: ${url}`);
    } catch (error) {
      if (attempt || /^HTTP 4/.test((error as Error).message)) throw error;
    }
  }
}
