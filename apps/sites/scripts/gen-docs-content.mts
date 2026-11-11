#!/usr/bin/env node
// gen-docs-content.mts -- vendor the decentraland/docs GitBook mirror into content/docs,
// the committed tree the /docs routes render (src/data/lib/docs, src/features/docs).
// Writes clean CommonMark per page, the SUMMARY.md navigation and page metadata
// (manifest.json, with the moved paths each space's .gitbook.yaml redirects), the full-site
// search index, every referenced asset and images the upstream Markdown loads from other hosts (_remote/). Raster images (PNG, JPEG, GIF) are
// replaced by one WebP each (x.png -> x.webp, _media.json); the originals are not copied.
// No HTML: the SSR routes render the Markdown.
//
// A second source covers the locale variants the live site serves and the mirror does not hold
// (docs-capture.mts): --capture reads the live sitemap index, fetches every variant page as
// Markdown and records the capture in _capture.json (date, sitemaps, each page's sha256, the
// images it loads). Runs without --capture keep the recorded capture as it is.
//
//   node scripts/gen-docs-content.mts             sync content/docs with the mirror and the recorded capture
//   node scripts/gen-docs-content.mts --capture   also refresh the capture from docs.decentraland.org
//   node scripts/gen-docs-content.mts --check     exit 1 when content/docs differs from what the mirror renders
//                                                 or from the recorded capture
//
// Env: SRC_DOCS (the decentraland/docs checkout, default ${DCL_MIRRORS:-~/github.com-decentraland}/docs),
// OUT (default content/docs), MAGICK, GIF2WEBP, TOKENCOUNT (a real tokenizer; token counts are omitted without one).
import { promises as fs } from "node:fs";
import fss from "node:fs";
import os from "node:os";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
import { validateDocs } from "./docs-validation.mts";
import { DocsMedia, rasterKind, webpName } from "./docs-images.mts";
import { RemoteImages } from "./docs-remote-images.mts";
import {
  buildNav, englishMatch, fetchText, groupTitles, LIVE, localUrl, mapBounded, parseSitemap,
  parseSitemapIndex, splitCaptured, translationsOf, unescapeUrls, type Variant, variantOf, variantSpace,
} from "./docs-capture.mts";

const SITES = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const CHECK = process.argv.includes("--check");
const CAPTURE = !CHECK && process.argv.includes("--capture");
const CAPTURE_LIMIT = 8;
const MIRRORS = process.env.DCL_MIRRORS || path.join(os.homedir(), "github.com-decentraland");
const SRC = path.resolve(process.env.SRC_DOCS || path.join(MIRRORS, "docs"));
const OUT = path.resolve(process.env.OUT || path.join(SITES, "content/docs"));
const BASE = "/docs";
const media = new DocsMedia({ output: OUT, base: BASE, readOnly: CHECK });
const remote = new RemoteImages({ output: OUT, mirrors: [MIRRORS, `${MIRRORS}-archived`], media, base: BASE, readOnly: CHECK });
const FAVICON = "player/images/favicon.ico";
const KEEP_ORIGINAL = new Set([FAVICON]);

const LEGACY_URLS: Record<string, string> = {
  "/docs/faqs/my-account/": "/docs/player/faqs/my-account/",
  "/docs/creator/scenes-sdk7/getting-started/vibe-coding/": "/docs/creator/sdk7/getting-started/vibe-coding/",
  "/docs/creator/scenes-sdk7/optimizing/scene-limitations/": "/docs/creator/sdk7/optimizing/scene-limitations/",
  "/docs/creator/development-guide/3d-model-animations/": "/docs/creator/sdk7/3d-essentials/3d-model-animations/",
  "/docs/creator/development-guide/sdk7/colliders/": "/docs/creator/sdk7/3d-essentials/colliders/",
  "/docs/creator/development-guide/scene-limitations/": "/docs/creator/sdk7/optimizing/scene-limitations/",
  "/docs/creator/emotes/avatar-rig/": "/docs/creator/wearables-and-emotes/emotes/avatar-rig/",
  "/docs/creator/development-guide/materials/": "/docs/creator/sdk7/3d-essentials/materials/",
  "/docs/decentraland/faq/": "/docs/player/faqs/decentraland-101/",
  "/docs/blockchain-integration/transactions-in-polygon/": "/docs/player/blockchain-integration/transactions-in-polygon/",
  "/docs/creator/scenes-sdk7/designing-the-experience/playtests/": "/docs/creator/sdk7/design-experience/playtests/",
  "/docs/creator/emotes/props-and-sounds/": "/docs/creator/wearables-and-emotes/emotes/props-and-sounds/",
  "/docs/creator/wearables/creating-wearables/": "/docs/creator/wearables-and-emotes/wearables/creating-wearables/",
  "/docs/creator/wearables/wearables-editor-user-guide/": "/docs/creator/wearables-and-emotes/publishing/publishing-collections/",
  "/docs/creator/development-guide/sdk7/smart-wearables/": "/docs/creator/sdk7/projects/smart-wearables/",
};
const FALLBACK = "creator/images/wearables-and-emotes/creating-wearables/19_skin_uv.webp";
const FALLBACK_SHA256 = "faf9d0eb4c552ab630da2d11095ff8dd527c6138d872fff357811fd3f16e3dbf";
const generatedPages = new Map<string, string>();

interface DocSpace {
  id: string; title: string; blurb: string; icon: "home" | "laptop" | "document" | "api" | "spark"; header?: boolean;
  path?: string; section?: string; language?: string; label?: string;
  source?: "mirror" | "capture"; revision?: string; capturedAt?: string; sitemap?: string | null;
}
// The live sitemap of each English space (docs.decentraland.org/sitemap.xml).
const LIVE_SITEMAP: Record<string, string> = {
  player: `${LIVE}/sitemap-pages.xml`, creator: `${LIVE}/creator/sitemap-pages.xml`,
  contributor: `${LIVE}/contributor/sitemap-pages.xml`, apis: `${LIVE}/apis/sitemap-pages.xml`,
};

const SPACES: DocSpace[] = [
  { id: "player", title: "Players", blurb: "Play, avatars, marketplace, DAO, wallets.", icon: "home", header: true },
  { id: "creator", title: "Content Creators", blurb: "Scene Editor, SDK7, wearables, emotes, 3D.", icon: "laptop", header: true },
  { id: "contributor", title: "Contributor", blurb: "Architecture, auth, content, runtime, comms.", icon: "document", header: true },
  { id: "apis", title: "APIs", blurb: "Backend service REST references.", icon: "api", header: true },
  { id: "ai-sdk-context", title: "AI SDK Context", blurb: "SDK7 complete reference for agents.", icon: "spark" },
];

const ASSET_RE = /\.(png|jpe?g|gif|svg|webp|avif|ico|mp4|webm|mov|glb|gltf|bin|fbx|obj|pdf|zip|json|wasm|mp3|wav|ogg|aac|txt|csv|xml|otf|ttf|woff2?|blend|ma|mb|pkr|max|c4d|abc|usdz|dae|3ds|psd|ai|apk|dmg|exe|unitypackage)$/i;
const referencedAssets = new Set<string>();
// Source raster -> the WebP that replaces it (output-relative); one name per source.
const rasters = new Map<string, string>();
const rasterNames = new Map<string, string>();
const collisions = new Map<string, string>();
const PAGE_URLS = new Set<string>();
const PAGE_BY_TAIL = new Map<string, string[]>();
interface SearchEntry { title: string; url: string; space: string; description: string; text: string }
const searchIndex: SearchEntry[] = [];
interface NavNode { type?: "section" | "item"; title?: string; url?: string | null; icon?: string | null; children: NavNode[] }
interface PageEntry {
  url: string; file: string; title: string; description: string | null; icon: string | null; space: string;
  source: string | null; updatedAt: string | null; navUrl?: string; tokens?: number; alternates?: Record<string, string>;
}
const pages: PageEntry[] = [];
const drift: string[] = [];
const expected = new Set<string>();

const TOKENCOUNT = process.env.TOKENCOUNT || "";
function countTokens(md: string): number | undefined {
  if (!TOKENCOUNT) return undefined;
  try {
    const n = parseInt((execFileSync(TOKENCOUNT, { input: md, encoding: "utf8", maxBuffer: 32 * 1024 * 1024 }).match(/\d+/) || [])[0] ?? "", 10);
    return Number.isFinite(n) ? n : undefined;
  } catch {
    return undefined;
  }
}

function searchableText(md: string): string {
  return md
    .replace(/```[\s\S]*?```|~~~[\s\S]*?~~~/g, " ")
    .replace(/!?(?:\[([^\]]*)\])\([^)]*\)/g, "$1")
    .replace(/<[^>]+>|[`*_>#{}|]/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 5000);
}

function splitHash(t: string): [string, string] {
  const i = t.indexOf("#");
  return i === -1 ? [t, ""] : [t.slice(0, i), t.slice(i + 1)];
}

function cleanUrl(srcRel: string): string {
  let p = String(srcRel).replace(/\\/g, "/").replace(/^\.\//, "").replace(/\/+$/, "");
  if (/\.md$/i.test(p)) p = p.slice(0, -3);
  p = p.replace(/(^|\/)readme$/i, "$1").replace(/^\/+|\/+$/g, "");
  return p ? `${BASE}/${p}/` : `${BASE}/`;
}

function resolveLink(fromRel: string, target: string, fuzzy = false): string {
  if (!target) return target;
  const trimmed = target.trim();
  if (trimmed.startsWith("/docs/")) return target;
  if (trimmed.startsWith("<") && trimmed.endsWith(">")) return "<" + resolveLink(fromRel, trimmed.slice(1, -1), fuzzy) + ">";
  const [rawPath, hash] = splitHash(trimmed);
  const anchor = hash ? "#" + hash : "";
  if (/^https?:\/\//i.test(rawPath)) {
    const m = rawPath.match(/^https?:\/\/docs\.decentraland\.org\/?(.*)$/i);
    if (!m) return target;
    const url = cleanUrl(m[1] || "");
    if (url === "/docs/decentraland/faq/" && (hash === "what-is-land" || hash === "what-is-an-estate")) return LEGACY_URLS[url];
    return (LEGACY_URLS[url] ?? url) + anchor;
  }
  if (/^[a-z][a-z0-9+.-]*:/i.test(rawPath) || rawPath === "" || rawPath.startsWith("#")) return target;
  const clean = rawPath.replace(/\\([_*()[\]#~.-])/g, "$1");
  const abs = clean.startsWith("/")
    ? clean.replace(/^\/+/, "")
    : path.posix.normalize(path.posix.join(path.posix.dirname(fromRel), clean)).replace(/^(\.\.\/)+/, "");
  if (abs.includes(".gitbook/assets/") || ASSET_RE.test(abs)) {
    const real = findAsset(abs, fromRel);
    const webp = replacement(real);
    if (webp) return `${BASE}/${webp}${anchor}`;
    referencedAssets.add(real);
    return `${BASE}/${real}${anchor}`;
  }
  if (clean.startsWith("/") && !pageExists(abs)) {
    const space = fromRel.split("/")[0];
    for (const cand of [`${space}/${abs}`, ...SPACES.map((s) => `${s.id}/${abs}`)])
      if (pageExists(cand)) return cleanUrl(cand) + anchor;
  }
  let url = cleanUrl(abs);
  if (fuzzy && PAGE_URLS.size && !PAGE_URLS.has(url)) url = fuzzyPage(url, fromRel) ?? url;
  return (LEGACY_URLS[url] ?? url) + anchor;
}

// The WebP name for a raster source asset, or null for anything served as-is. A name
// already taken (x.png and x.gif, or an upstream x.webp) keeps the source extension.
function replacement(real: string): string | null {
  if (KEEP_ORIGINAL.has(real)) return null;
  const known = rasters.get(real);
  if (known) return known;
  const file = path.join(SRC, real);
  let head: Buffer;
  try {
    const fd = fss.openSync(file, "r");
    try { head = Buffer.alloc(12); fss.readSync(fd, head, 0, 12, 0); } finally { fss.closeSync(fd); }
  } catch { return null; }
  if (!rasterKind(head)) return null;
  let name = webpName(real);
  if ((rasterNames.has(name) && rasterNames.get(name) !== real) || (name !== real && fss.existsSync(path.join(SRC, name)))) name = `${real}.webp`;
  rasters.set(real, name);
  rasterNames.set(name, real);
  return name;
}

function fuzzyPage(url: string, fromRel: string): string | null {
  const segs = url.slice(BASE.length).replace(/^\/+|\/+$/g, "").split("/").filter(Boolean);
  if (!segs.length) return null;
  const rest = segs.slice(1);
  for (const sp of new Set([segs[0], fromRel.split("/")[0]])) {
    for (let n = Math.min(3, rest.length); n >= 2; n--) {
      const hits = PAGE_BY_TAIL.get(`${sp}|${rest.slice(-n).join("/")}`);
      if (hits && hits.length === 1) return hits[0];
    }
  }
  return null;
}

// GitBook declares moved pages in a space's .gitbook.yaml `redirects:` map: live path -> page file.
function gitbookRedirects(yaml: string): [string, string][] {
  const out: [string, string][] = [];
  let inside = false;
  for (const line of yaml.split("\n")) {
    if (/^\S/.test(line)) { inside = /^redirects:/.test(line); continue; }
    const m = inside ? line.match(/^\s+([^\s#:][^:]*?)\s*:\s*(\S+)\s*$/) : null;
    if (m) out.push([m[1].replace(/^["']|["']$/g, ""), m[2].replace(/^["']|["']$/g, "")]);
  }
  return out;
}

// The redirect source URLs under /docs with the mirror page each one names; a source that is
// itself a page, or a target that is no page (the file's case excepted), is left out.
async function declaredRedirects(): Promise<Record<string, string>> {
  const byLower = new Map([...PAGE_URLS].map((u) => [u.toLowerCase(), u]));
  const redirects: Record<string, string> = {};
  for (const space of SPACES) {
    const yaml = await fs.readFile(path.join(SRC, space.id, ".gitbook.yaml"), "utf8").catch(() => null);
    if (!yaml) continue;
    for (const [from, to] of gitbookRedirects(yaml)) {
      const source = cleanUrl(`${space.id}/${from}`);
      const target = byLower.get(cleanUrl(`${space.id}/${to}`).toLowerCase());
      if (!target) { console.warn(`[docs-content] redirect to a missing page: ${space.id}/${from} -> ${to}`); continue; }
      if (!PAGE_URLS.has(source) && source !== target) redirects[source] = target;
    }
  }
  return Object.fromEntries(Object.entries(redirects).sort(([a], [b]) => (a < b ? -1 : 1)));
}

function pageExists(p: string): boolean {
  const q = p.replace(/\/+$/, "");
  return fss.existsSync(path.join(SRC, q + ".md")) || fss.existsSync(path.join(SRC, q, "README.md")) || fss.existsSync(path.join(SRC, q));
}

let ASSET_INDEX: Map<string, string[]> | null = null;
function assetIndex(): Map<string, string[]> {
  if (ASSET_INDEX) return ASSET_INDEX;
  ASSET_INDEX = new Map();
  for (const full of walkFiles(SRC, (name) => ASSET_RE.test(name), (name) => name === ".git")) {
    const name = path.basename(full), rel = path.relative(SRC, full).replace(/\\/g, "/");
    if (!ASSET_INDEX.has(name)) ASSET_INDEX.set(name, []);
    ASSET_INDEX.get(name)!.push(rel);
  }
  for (const list of ASSET_INDEX.values()) list.sort();
  return ASSET_INDEX;
}
function findAsset(abs: string, fromRel: string): string {
  if (fss.existsSync(path.join(SRC, abs))) return abs;
  const space = fromRel.split("/")[0];
  for (const t of [`${space}/${abs}`, ...SPACES.map((s) => `${s.id}/${abs}`)])
    if (fss.existsSync(path.join(SRC, t))) return t;
  const hits = assetIndex().get(path.posix.basename(abs)) || [];
  return hits.find((h) => h.startsWith(space + "/")) || hits[0] || abs;
}

function stashCode(md: string): { text: string; restore: (s: string) => string } {
  const stash: string[] = [];
  const keep = (m: string) => `\x00${stash.push(m) - 1}\x00`;
  const text = md.replace(/<!--[\s\S]*?-->/g, keep).replace(/```[\s\S]*?```/g, keep).replace(/~~~[\s\S]*?~~~/g, keep).replace(/`[^`\n]*`/g, keep);
  return { text, restore: (s) => s.replace(/\x00(\d+)\x00/g, (_m, i) => stash[Number(i)]) };
}

const MD_IMAGE = /!\[((?:[^\]\\]|\\.)*)\]\(\s*<?(https?:\/\/[^)\s>]+)>?([^)]*)\)/g;
const HTML_IMG = /<img\b[^>]*>/gi;
const SRC_ATTR = /\bsrc=(?:"([^"]*)"|'([^']*)')/i;

function remoteImageUrls(md: string): string[] {
  const { text } = stashCode(md);
  const out: string[] = [];
  for (const m of text.matchAll(MD_IMAGE)) out.push(m[2]);
  for (const tag of text.match(HTML_IMG) ?? []) {
    const src = tag.match(SRC_ATTR);
    const url = src?.[1] ?? src?.[2];
    if (url && /^https?:\/\//i.test(url)) out.push(url);
  }
  return out.filter((u) => !/^https?:\/\/docs\.decentraland\.org\//i.test(u));
}

function escAttr(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");
}

// Remote images become their vendored copy; one that could not be fetched degrades
// to a plain link, so the page never loads an image from another host.
function vendorRemoteImages(s: string): string {
  s = s.replace(MD_IMAGE, (m, alt, url, tail) => {
    if (/^https?:\/\/docs\.decentraland\.org\//i.test(url)) return m;
    const local = remote.local(url);
    return local ? `![${alt}](${local}${tail})` : `[${alt || "image"}](${url})`;
  });
  return s.replace(HTML_IMG, (tag) => {
    const src = tag.match(SRC_ATTR);
    const url = src?.[1] ?? src?.[2];
    if (!url || !/^https?:\/\//i.test(url) || /^https?:\/\/docs\.decentraland\.org\//i.test(url)) return tag;
    const local = remote.local(url);
    if (local) return tag.replace(SRC_ATTR, `src="${escAttr(local)}"`);
    const alt = tag.match(/\balt=(?:"([^"]*)"|'([^']*)')/i);
    return `<a href="${escAttr(url)}">${alt?.[1] ?? alt?.[2] ?? "image"}</a>`;
  });
}

// Rewrite links/images/HTML attrs in markdown, skipping code so examples stay intact.
function rewriteLinks(md: string, resolve: (target: string) => string): string {
  const { text, restore } = stashCode(md);
  let s = text.replace(/(!?\[(?:[^\]\\]|\\.)*\])\(\s*(<[^>]*>|[^)\s]+)([^)]*)\)/g,
    (_m, label, url, tail) => `${label}(${resolve(url)}${tail})`);
  s = s.replace(/^(\s*\[[^\]]+\]:\s*)(\S+)/gm, (_m, pre, url) => `${pre}${resolve(url)}`);
  s = s.replace(/\b(src|href|poster|data-src)=("([^"]*)"|'([^']*)')/gi,
    (_m, attr, _q, dq, sq) => `${attr}="${resolve(dq ?? sq ?? "")}"`);
  return restore(vendorRemoteImages(s));
}
const mirrorLinks = (fromRel: string) => (target: string) => resolveLink(fromRel, target, true);

function repairSource(body: string): string {
  return body
    .replace(/\\\x01src="\.\.\/images\/wearables-and-emotes\/creating-wearables\/19\\_skin\\_uv\.png"width="600"\/>/g,
      `![Avatar skin UV map](/docs/${FALLBACK})`)
    .replace(/!\[\x01\]/g, "![]")
    .replace(/alt="\x01"/g, 'alt=""')
    .replace(/https:\/\/github\.com\/lordlikedao\/documentation\/blob\/main\/static\/images\/DAO%20Organizational%20Chart\/Revocation\.png\?raw=true/g,
      "/docs/player/images/DAO%20Organizational%20Chart/Revocation.webp");
}

function stripFrontMatter(raw: string): { fm: string; body: string } {
  if (!raw.startsWith("---")) return { fm: "", body: raw };
  const end = raw.indexOf("\n---", 3);
  if (end === -1) return { fm: "", body: raw };
  const nl = raw.indexOf("\n", end + 1);
  return { fm: raw.slice(3, end), body: nl === -1 ? "" : raw.slice(nl + 1) };
}
function fmField(fm: string, key: string): string | null {
  const m = fm.match(new RegExp(`^${key}:\\s*(.+)$`, "m"));
  return m ? m[1].trim().replace(/^["']|["']$/g, "") : null;
}

// YouTube embeds keep their click-to-load placeholder (src/features/docs wires the button);
// the placeholder itself makes no YouTube request.
function embeds(md: string): string {
  return md
    .replace(/\{%\s*embed url="([^"]+)"[^%]*%\}/g, (_m, url) => {
      const video = url.match(/(?:youtube\.com\/watch\?v=|youtu\.be\/)([\w-]+)/i);
      const playlist = url.match(/[?&]list=([\w-]+)/i);
      if (video || playlist) {
        const src = video ? `https://www.youtube-nocookie.com/embed/${video[1]}` : `https://www.youtube-nocookie.com/embed/videoseries?list=${playlist[1]}`;
        return `\n\n<div class="video-embed"><button type="button" class="video-play" data-video-src="${escAttr(src)}" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>\n<p class="video-source"><a href="${escAttr(url)}">Watch on YouTube</a></p>\n\n`;
      }
      return `\n\n[${url}](${url})\n\n`;
    })
    .replace(/\{%\s*endembed\s*%\}/g, "");
}

// GitBook hints become GitHub alerts; the renderer turns them back into styled hints.
function hintsToBlockquote(md: string): string {
  const LABEL: Record<string, string> = { info: "NOTE", warning: "WARNING", danger: "CAUTION", success: "TIP" };
  const out: string[] = [];
  let inHint = false;
  for (const line of md.split("\n")) {
    const open = line.match(/^[ \t]*\{%\s*hint style="(\w+)"\s*%\}[ \t]*$/);
    const close = line.match(/^[ \t]*\{%\s*endhint\s*%\}[ \t]*$/);
    if (open) { inHint = true; out.push("", `> [!${LABEL[open[1]] || "NOTE"}]`); continue; }
    if (close) { inHint = false; out.push(""); continue; }
    out.push(inHint ? (line.trim() ? "> " + line : ">") : line);
  }
  return out.join("\n");
}

function summaryIcon(space: string, target: string | null): string | null {
  if (!target || /^[a-z][a-z0-9+.-]*:/i.test(target)) return null;
  const clean = splitHash(target.replace(/^<|>$/g, ""))[0];
  let file = path.join(SRC, path.posix.normalize(path.posix.join(space, clean)));
  if (fss.existsSync(file) && fss.statSync(file).isDirectory()) file = path.join(file, "README.md");
  if (!fss.existsSync(file) && !path.extname(file)) file += ".md";
  try {
    return fmField(stripFrontMatter(fss.readFileSync(file, "utf8")).fm, "icon");
  } catch {
    return null;
  }
}

function parseSummary(space: string, text: string): NavNode {
  const root: NavNode = { children: [] };
  const stack: Array<{ indent: number; node: NavNode }> = [{ indent: -1, node: root }];
  let inFence = false;
  for (const line of text.split("\n")) {
    // GitBook nests fenced ```yaml OpenAPI blocks under list items; skip fenced
    // content so the fence/body never leaks into the nav (open+close both toggle).
    if (/^\s*(?:[*-]\s+)?```/.test(line)) { inFence = !inFence; continue; }
    if (inFence) continue;
    const sec = line.match(/^##\s+(.+?)\s*$/);
    if (sec) {
      const node: NavNode = { type: "section", title: sec[1], children: [] };
      root.children.push(node);
      stack.length = 0;
      stack.push({ indent: -1, node });
      continue;
    }
    const it = line.match(/^([ \t]*)[*-]\s+(?:\[(.+?)\]\((.+?)\)|(.+?))\s*$/);
    if (!it) continue;
    const indent = it[1].replace(/\t/g, "  ").length;
    const title = (it[2] ?? it[4] ?? "").trim();
    const target = it[3] ?? null;
    if (!title) continue;
    const node: NavNode = { type: "item", title, url: target ? resolveLink(`${space}/SUMMARY.md`, target) : null, icon: summaryIcon(space, target), children: [] };
    while (stack.length && stack[stack.length - 1].indent >= indent) stack.pop();
    (stack.length ? stack[stack.length - 1].node : root).children.push(node);
    stack.push({ indent, node });
  }
  return root;
}

// Nav entries whose target is not a rendered page stay as plain labels.
function pruneNav(node: NavNode, pageUrls: Set<string>): NavNode {
  return {
    ...node,
    url: node.url && pageUrls.has(node.url) ? node.url : null,
    children: node.children.map((c) => pruneNav(c, pageUrls)),
  };
}

let gitDates: Map<string, string> | null = null;
function sourceUpdatedAt(srcRel: string): string | null {
  if (gitDates === null) {
    gitDates = new Map();
    try {
      const log = execFileSync("git", ["-C", SRC, "log", "--format=@@%cI", "--name-only", "--", "."], { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
      let date = "";
      for (const line of log.split("\n")) {
        if (line.startsWith("@@")) date = line.slice(2);
        else if (date && line && !gitDates.has(line)) gitDates.set(line, date);
      }
    } catch {}
  }
  return gitDates.get(srcRel) || null;
}

function walkFiles(dir: string, include: (name: string) => boolean, skip: (name: string) => boolean): string[] {
  const out: string[] = [];
  const stack = [dir];
  while (stack.length) {
    const d = stack.pop()!;
    let ents;
    try { ents = fss.readdirSync(d, { withFileTypes: true }); } catch { continue; }
    for (const e of ents) {
      const full = path.join(d, e.name);
      if (e.isDirectory()) { if (!skip(e.name)) stack.push(full); }
      else if (e.isFile() && include(e.name)) out.push(full);
    }
  }
  return out;
}
function walkMd(dir: string): string[] {
  return walkFiles(dir, (name) => name.toLowerCase().endsWith(".md") && name !== "SUMMARY.md", (name) => name === ".gitbook" || name === ".git").sort();
}

// The committed tree is synced, never wiped: unchanged files are left alone, so a re-run
// only touches what the mirror changed. --check compares instead of writing.
async function writeText(rel: string, content: string): Promise<void> {
  expected.add(rel);
  const file = path.join(OUT, rel);
  const current = await fs.readFile(file, "utf8").catch(() => null);
  if (current === content) return;
  if (CHECK) { drift.push(`${current === null ? "missing" : "differs"}: ${rel}`); return; }
  await fs.mkdir(path.dirname(file), { recursive: true });
  await fs.writeFile(file, content);
}

async function copyAsset(rel: string): Promise<boolean> {
  const from = path.join(SRC, rel), to = path.join(OUT, rel);
  const src = await fs.stat(from).catch(() => null);
  if (!src?.isFile()) return false;
  expected.add(rel);
  const dst = await fs.stat(to).catch(() => null);
  if (dst && dst.size === src.size && (await fs.readFile(to)).equals(await fs.readFile(from))) return true;
  if (CHECK) { drift.push(`${dst ? "differs" : "missing"}: ${rel}`); return true; }
  await fs.mkdir(path.dirname(to), { recursive: true });
  await fs.copyFile(from, to);
  return true;
}

function pageFile(url: string): string {
  const rel = url.slice(BASE.length).replace(/^\/+|\/+$/g, "");
  return rel ? `${rel}/index.md` : "index.md";
}

async function addPage(entry: Omit<PageEntry, "file" | "tokens">, md: string): Promise<void> {
  const file = pageFile(entry.url);
  generatedPages.set(entry.url, md);
  const tokens = countTokens(md);
  pages.push({ ...entry, file, ...(tokens === undefined ? {} : { tokens }) });
  await writeText(file, md);
}

async function writePage(srcRel: string): Promise<void> {
  const { fm, body } = stripFrontMatter(await fs.readFile(path.join(SRC, srcRel), "utf8"));
  let cleaned = rewriteLinks(embeds(repairSource(body)), mirrorLinks(srcRel)).trim();
  const title = fmField(fm, "title") || (cleaned.match(/^#\s+(.+)$/m)?.[1]?.trim()) || path.basename(srcRel, ".md");
  const description = fmField(fm, "description");
  if (!/^#\s+/.test(cleaned)) cleaned = `# ${title}\n\n${cleaned}`;
  const url = cleanUrl(srcRel);
  if (collisions.has(url) && collisions.get(url) !== srcRel)
    console.warn(`[docs-content] url collision ${url}: ${collisions.get(url)} vs ${srcRel}`);
  collisions.set(url, srcRel);
  const space = SPACES.find((item) => item.id === srcRel.split("/")[0]) || SPACES[0];
  searchIndex.push({ title, url, space: space.title, description: description || "", text: searchableText(cleaned) });
  const md = (description ? `> ${description}\n\n` : "") + hintsToBlockquote(cleaned) + "\n";
  await addPage({ url, title, description, icon: fmField(fm, "icon"), space: space.id, source: srcRel, updatedAt: sourceUpdatedAt(srcRel) }, md);
}

function navToMarkdownList(node: NavNode, depth = 0): string {
  let md = "";
  for (const c of node.children || []) {
    const indent = "  ".repeat(depth);
    if (c.type === "section") md += `\n${indent}**${c.title}**\n\n` + navToMarkdownList(c, depth);
    else {
      md += c.url ? `${indent}- [${c.title}](${c.url})\n` : `${indent}- ${c.title}\n`;
      md += navToMarkdownList(c, depth + 1);
    }
  }
  return md;
}

// A space without README.md still gets a landing page at /docs/<space>/.
async function writeSpaceLanding(space: DocSpace, navTree: NavNode): Promise<void> {
  const url = `${BASE}/${space.id}/`;
  if (collisions.has(url)) return;
  const title = `${space.title} Documentation`, bodyMd = `# ${title}\n\n${space.blurb}\n\n${navToMarkdownList(navTree)}`;
  searchIndex.push({ title, url, space: space.title, description: space.blurb, text: searchableText(bodyMd) });
  await addPage({ url, title, description: space.blurb, icon: null, space: space.id, source: null, updatedAt: null }, bodyMd + "\n");
  collisions.set(url, `synthetic:${space.id}`);
}

// Pre-pass: know every page URL (and its path-tails) before rendering, so content
// links can fuzzy-resolve to real pages anywhere in the tree.
function indexPages(): void {
  PAGE_URLS.add(`${BASE}/`);
  for (const space of SPACES) {
    const spaceDir = path.join(SRC, space.id);
    if (!fss.existsSync(spaceDir)) continue;
    const urls = walkMd(spaceDir).map((f) => cleanUrl(path.relative(SRC, f).replace(/\\/g, "/")));
    urls.push(`${BASE}/${space.id}/`);
    for (const url of urls) {
      PAGE_URLS.add(url);
      const rest = url.slice(BASE.length).replace(/^\/+|\/+$/g, "").split("/").filter(Boolean).slice(1);
      for (let n = 2; n <= Math.min(3, rest.length); n++) {
        const key = `${space.id}|${rest.slice(-n).join("/")}`;
        if (!PAGE_BY_TAIL.has(key)) PAGE_BY_TAIL.set(key, []);
        PAGE_BY_TAIL.get(key)!.push(url);
      }
    }
  }
}

async function pruneStale(): Promise<void> {
  const present = walkFiles(OUT, () => true, () => false).map((f) => path.relative(OUT, f).split(path.sep).join("/"));
  for (const rel of present.filter((rel) => !expected.has(rel)).sort()) {
    if (CHECK) { drift.push(`stale: ${rel}`); continue; }
    await fs.rm(path.join(OUT, rel));
  }
  if (CHECK) return;
  const removeEmpty = async (dir: string): Promise<boolean> => {
    let empty = true;
    for (const e of await fs.readdir(dir, { withFileTypes: true })) {
      if (!e.isDirectory() || !(await removeEmpty(path.join(dir, e.name)))) empty = false;
    }
    if (empty && dir !== OUT) await fs.rmdir(dir);
    return empty;
  };
  await removeEmpty(OUT);
}

const CAPTURE_FILE = "_capture.json";
interface CapturedPage extends PageEntry { live: string; sha256: string; en: string | null }
interface CapturedVariant {
  variant: Variant; capturedAt: string; live: number; groups: string[]; nav: NavNode; pages: CapturedPage[]; images: string[];
}
interface CaptureRecord {
  index: string; capturedAt: string; variants: CapturedVariant[];
  skipped: Array<{ sitemap: string; reason: string }>;
}
interface RawVariant { variant: Variant; capturedAt: string; groups: string[]; entries: Array<{ loc: string; lastmod: string | null; md: string }> }

const sha256 = (s: string) => createHash("sha256").update(s).digest("hex");
const spaceOf = (id: string) => SPACES.find((s) => s.id === id)!;

function readCapture(): CaptureRecord | null {
  try { return JSON.parse(fss.readFileSync(path.join(OUT, CAPTURE_FILE), "utf8")) as CaptureRecord; } catch { return null; }
}

async function fetchCapture(): Promise<{ index: string; raw: RawVariant[]; skipped: CaptureRecord["skipped"]; enLive: Record<string, string[]> }> {
  const index = `${LIVE}/sitemap.xml`;
  const raw: RawVariant[] = [];
  const skipped: CaptureRecord["skipped"] = [];
  for (const sitemap of parseSitemapIndex(await fetchText(index, "application/xml"))) {
    const variant = variantOf(sitemap);
    if (!variant) {
      if (!Object.values(LIVE_SITEMAP).includes(sitemap)) skipped.push({ sitemap, reason: "not a known space or locale variant" });
      continue;
    }
    if (!SPACES.some((s) => s.id === variant.section)) { skipped.push({ sitemap, reason: `no English space ${variant.section}` }); continue; }
    if (walkMd(path.join(SRC, variant.path)).length) { skipped.push({ sitemap, reason: "the mirror holds this variant" }); continue; }
    const capturedAt = new Date().toISOString();
    const entries = parseSitemap(await fetchText(sitemap, "application/xml"));
    const home = `${LIVE}/${variant.path}`;
    const groups = groupTitles(await fetchText(`${home}/`, "text/html"));
    const bodies = await mapBounded(entries, CAPTURE_LIMIT, async (entry) => {
      const md = unescapeUrls(await fetchText(entry.loc.replace(/\/+$/, "") === home ? `${home}/readme.md` : `${entry.loc}.md`, "text/markdown"));
      if (/^# Page Not Found\n/.test(md)) throw new Error(`live page not found as Markdown: ${entry.loc}`);
      return md;
    });
    raw.push({ variant, capturedAt, groups, entries: entries.map((e, i) => ({ ...e, md: bodies[i] })) });
    console.log(`[docs-content] captured ${variant.path}: ${entries.length} pages`);
  }
  const enLive: Record<string, string[]> = {};
  for (const section of new Set(raw.map((r) => r.variant.section))) {
    enLive[section] = parseSitemap(await fetchText(LIVE_SITEMAP[section], "application/xml")).map((e) => e.loc);
  }
  return { index, raw, skipped, enLive };
}

const spaceUrls = (space: string) => pages.filter((p) => p.space === space).map((p) => p.url);

async function renderCapture(raw: RawVariant, enLive: string[], variants: Variant[]): Promise<CapturedVariant> {
  const { variant } = raw;
  const resolve = (target: string): string => {
    const local = localUrl(target, variants);
    if (local) return local;
    const live = target.match(/^(?:https?:\/\/docs\.decentraland\.org)?(\/(?:player|creator|contributor|apis)\/[^#)\s]*)(#\S*)?$/i);
    if (live) {
      const hit = englishMatch(live[1], spaceUrls(live[1].split("/")[1]));
      if (hit) return hit + (live[2] ?? "");
      return `${LIVE}${live[1].replace(/\.md$/i, "")}${live[2] ?? ""}`;
    }
    return target;
  };
  const out: CapturedPage[] = [];
  for (const [i, entry] of raw.entries.entries()) {
    const url = localUrl(entry.loc, [variant])!;
    // The English page a variant page translates, for the language switcher.
    const en = enLive[i] ? englishMatch(enLive[i], spaceUrls(variant.section)) : null;
    const enDescription = en ? pages.find((p) => p.url === en)?.description : null;
    const { title, description, body } = splitCaptured(entry.md, Boolean(enDescription));
    let cleaned = rewriteLinks(embeds(body), resolve).trim();
    if (!/^#\s+/.test(cleaned)) cleaned = `# ${title}\n\n${cleaned}`;
    const md = (description ? `> ${description}\n\n` : "") + hintsToBlockquote(cleaned) + "\n";
    const file = pageFile(url);
    await writeText(file, md);
    out.push({ url, file, title: title || url, description, icon: null, space: variant.id, source: null, updatedAt: entry.lastmod, live: entry.loc, sha256: sha256(md), en });
  }
  const nav = buildNav(`/docs/${variant.path}/`, out, raw.groups) as NavNode;
  const images = [...new Set(raw.entries.flatMap((e) => remoteImageUrls(e.md)))].sort();
  console.log(`[docs-content] ${variant.path}: ${out.length} pages, ${raw.groups.length} groups (${spaceOf(variant.section).title})`);
  return { variant, capturedAt: raw.capturedAt, live: raw.entries.length, groups: raw.groups, nav, pages: out, images };
}

// A recorded variant as it is: its files must still hold what the capture wrote.
async function keepCapture(v: CapturedVariant): Promise<void> {
  for (const page of v.pages) {
    expected.add(page.file);
    const current = await fs.readFile(path.join(OUT, page.file), "utf8").catch(() => null);
    if (current === null) drift.push(`missing (capture): ${page.file}`);
    else if (sha256(current) !== page.sha256) drift.push(`differs from the capture: ${page.file}`);
  }
}

async function main(): Promise<void> {
  if (!fss.existsSync(path.join(SRC, "creator"))) throw new Error(`source docs mirror not found (expected the GitBook monorepo): ${SRC}`);
  await fs.mkdir(OUT, { recursive: true });
  indexPages();

  const sources = SPACES.flatMap((space) => {
    const dir = path.join(SRC, space.id);
    return fss.existsSync(dir) ? walkMd(dir).map((f) => path.relative(SRC, f).replace(/\\/g, "/")) : [];
  });
  const remoteUrls = new Set<string>();
  for (const rel of sources) for (const url of remoteImageUrls(repairSource(stripFrontMatter(fss.readFileSync(path.join(SRC, rel), "utf8")).body))) remoteUrls.add(url);
  const recorded = readCapture();
  const fetched = CAPTURE ? await fetchCapture() : null;
  // A captured page keeps the text it was captured with, so an image the source repair replaces
  // for the mirror's pages (gone upstream) is not fetched for a capture either.
  const captured = fetched ? fetched.raw.flatMap((r) => r.entries.flatMap((e) => remoteImageUrls(e.md))) : (recorded?.variants ?? []).flatMap((v) => v.images);
  for (const url of captured) if (repairSource(url) === url) remoteUrls.add(url);
  await remote.vendor(remoteUrls);

  const nav: Record<string, NavNode> = {};
  for (const space of SPACES) {
    const spaceDir = path.join(SRC, space.id);
    if (!fss.existsSync(spaceDir)) { console.warn(`[docs-content] missing space ${space.id}`); continue; }
    const files = walkMd(spaceDir).map((f) => path.relative(SRC, f).replace(/\\/g, "/"));
    const summaryPath = path.join(spaceDir, "SUMMARY.md");
    const navTree: NavNode = fss.existsSync(summaryPath)
      ? parseSummary(space.id, await fs.readFile(summaryPath, "utf8"))
      : { children: files.map((f): NavNode => ({ type: "item", title: path.basename(f, ".md"), url: cleanUrl(f), children: [] })) };
    const pageUrls = new Set(files.map(cleanUrl));
    pageUrls.add(`${BASE}/${space.id}/`);
    nav[space.id] = pruneNav(navTree, pageUrls);
    for (const f of files) await writePage(f);
    await writeSpaceLanding(space, nav[space.id]);
  }

  // GitBook exposes the Player space at its site root; /docs/player/ stays as the
  // compatibility alias while /docs/ renders the same page with the Player nav.
  const rootParts = stripFrontMatter(await fs.readFile(path.join(SRC, "player/README.md"), "utf8"));
  const rootBody = rewriteLinks(embeds(repairSource(rootParts.body)), mirrorLinks("player/README.md")).trim();
  await addPage({
    url: `${BASE}/`, title: "Welcome", description: "Everything you need to know to show up, hang out, and be part of Decentraland.",
    icon: fmField(rootParts.fm, "icon"), space: "player", source: "player/README.md", updatedAt: sourceUpdatedAt("player/README.md"), navUrl: `${BASE}/player/`,
  }, hintsToBlockquote(rootBody) + "\n");
  const homeSearch = searchIndex.find((entry) => entry.url === `${BASE}/player/`);
  if (homeSearch) searchIndex.unshift({ ...homeSearch, url: `${BASE}/` });

  let capture: CaptureRecord | null = recorded;
  if (fetched) {
    const rendered: CapturedVariant[] = [];
    for (const r of fetched.raw) rendered.push(await renderCapture(r, fetched.enLive[r.variant.section] ?? [], fetched.raw.map((x) => x.variant)));
    const capturedAt = rendered.map((v) => v.capturedAt).sort()[0] ?? new Date().toISOString();
    capture = { index: fetched.index, capturedAt, variants: rendered, skipped: fetched.skipped };
  }
  const variants = capture?.variants ?? [];
  if (!fetched) for (const v of variants) await keepCapture(v);
  for (const v of variants) {
    nav[v.variant.id] = v.nav;
    for (const { live: _live, sha256: _sha, en: _en, ...page } of v.pages) pages.push(page);
  }
  const translations = translationsOf(variants);
  for (const page of pages) {
    const alt = translations.get(page.url);
    if (alt) page.alternates = Object.fromEntries(Object.entries(alt).sort(([a], [b]) => (a < b ? -1 : 1)));
  }

  referencedAssets.add(FAVICON);
  referencedAssets.add("player/images/DAO Organizational Chart/Revocation.webp");
  expected.add(FALLBACK);
  const fallback = await fs.readFile(path.join(OUT, FALLBACK)).catch(() => null);
  if (!fallback || createHash("sha256").update(fallback).digest("hex") !== FALLBACK_SHA256) drift.push(`missing or altered pinned image: ${FALLBACK}`);
  else media.entries.set(`${BASE}/${FALLBACK}`, { source: "creator/images/wearables-and-emotes/creating-wearables/19_skin_uv.png", encoder: "docs-media-v3-s1600q82l16384-g2w-q70r0.6-a1024:", key: "29b46ad7520413bbdc2bac62", width: 1600, height: 815, bytes: 54890, sourceBytes: 832188 });
  let copied = 0;
  for (const rel of [...referencedAssets].sort()) {
    if (await copyAsset(rel)) copied++;
    else drift.push(`missing source asset: ${rel}`);
  }
  for (const [real, webp] of [...rasters.entries()].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))) {
    media.replace(fss.readFileSync(path.join(SRC, real)), webp, real);
  }
  await media.flush();
  for (const rel of [...media.files(), ...remote.files()]) expected.add(rel);
  for (const url of media.missing) drift.push(`webp not built: ${url}`);
  for (const url of remote.missing) drift.push(`remote image not vendored: ${url}`);

  const commit = execFileSync("git", ["-C", SRC, "rev-parse", "HEAD"], { encoding: "utf8" }).trim();
  const committedAt = execFileSync("git", ["-C", SRC, "log", "-1", "--format=%cI"], { encoding: "utf8" }).trim();
  pages.sort((a, b) => (a.url < b.url ? -1 : a.url > b.url ? 1 : 0));
  const manifest = {
    generator: "sites/scripts/gen-docs-content.mts",
    source: { repo: "decentraland/docs", commit, committedAt, fallbackMedia: [{ file: FALLBACK, sourceCommit: "452eabb509a8a1806d3c503a94adbb886711e53f", sourceFile: "creator/images/wearables-and-emotes/creating-wearables/19_skin_uv.png", sourceSha256: "25cf00f8fb30012e1e8cd8eb646a55cbafb744c2dcec009adecc3161fb2f0493", sha256: FALLBACK_SHA256 }] },
    spaces: [
      ...SPACES.map((s): DocSpace => ({ ...s, language: "en", label: "EN", source: "mirror", revision: commit, sitemap: LIVE_SITEMAP[s.id] ?? null })),
      ...variants.map((v): DocSpace => ({ ...variantSpace(v.variant, spaceOf(v.variant.section), v.capturedAt), source: "capture" }))
    ],
    nav,
    redirects: await declaredRedirects(),
    pages,
  };
  await writeText("manifest.json", JSON.stringify(manifest, null, 1) + "\n");
  await writeText("search-index.json", JSON.stringify(searchIndex));
  await writeText("_media.json", media.manifest());
  await writeText("_remote/sources.json", remote.manifest());
  for (const problem of validateDocs(generatedPages, expected)) drift.push(problem);
  if (capture) await writeText(CAPTURE_FILE, JSON.stringify(capture, null, 1) + "\n");
  const failed = [...remote.entries.entries()].filter(([, e]) => "error" in e);
  for (const [url, e] of failed) drift.push(`remote image not vendored (${"error" in e ? e.error : ""}): ${url}`);
  if (!CHECK && drift.length) throw new Error(`invalid generated docs:\n${drift.join("\n")}`);
  await pruneStale();
  if (CHECK) {
    if (drift.length) {
      console.error(`[docs-content] content/docs is stale (${drift.length}):\n  ${drift.slice(0, 40).join("\n  ")}${drift.length > 40 ? "\n  ..." : ""}`);
      console.error("[docs-content] regenerate: node scripts/gen-docs-content.mts");
      process.exit(1);
    }
    console.log(`[docs-content] content/docs matches ${SRC} @ ${commit.slice(0, 12)} and the capture of ${capture?.capturedAt ?? "(none)"} (${pages.length} pages)`);
    return;
  }
  if (drift.length) throw new Error(`invalid generated docs:\n${drift.join("\n")}`);
  console.log(`[docs-content] ${pages.length} pages, ${copied} assets, ${rasters.size} images replaced by webp, ${remote.files().length} remote images (${failed.length} unreachable) -> ${path.relative(process.cwd(), OUT) || OUT}`);
}

main().catch((e) => { console.error(e); process.exit(1); });
