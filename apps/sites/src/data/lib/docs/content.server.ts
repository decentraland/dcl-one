// The committed docs.decentraland.org mirror (content/docs, scripts/gen-docs-content.mts):
// URL resolution, page views and the raw file and Markdown responses under /docs.
import fs from "node:fs";
import path from "node:path";
import { Readable } from "node:stream";

import { markdownResponse, wantsMarkdown } from "../agent/markdown";
import type { DocsMediaEntry } from "./images";
import { renderDocsPage } from "./render";
import type { DocsLink, DocsManifest, DocsNavNode, DocsPageEntry, DocsPageView } from "./types";

export const DOCS_BASE = "/docs";

/** content/docs beside the app (sites/ in a checkout, the package root in the nix build); DOCS_CONTENT_DIR overrides. */
export function docsContentDir(): string {
  return path.resolve(process.env.DOCS_CONTENT_DIR || "content/docs");
}

interface Loaded {
  dir: string;
  mtimeMs: number;
  manifest: DocsManifest;
  byUrl: Map<string, DocsPageEntry>;
  images: Record<string, DocsMediaEntry>;
  views: Map<string, DocsPageView>;
}
let loaded: Loaded | null = null;

function load(): Loaded | null {
  const dir = docsContentDir();
  const file = path.join(dir, "manifest.json");
  let mtimeMs: number;
  try {
    mtimeMs = fs.statSync(file).mtimeMs;
  } catch {
    return null;
  }
  if (loaded && loaded.dir === dir && loaded.mtimeMs === mtimeMs) return loaded;
  const manifest = JSON.parse(fs.readFileSync(file, "utf8")) as DocsManifest;
  let images: Record<string, DocsMediaEntry> = {};
  try {
    images = JSON.parse(fs.readFileSync(path.join(dir, "_media.json"), "utf8"));
  } catch {}
  loaded = { dir, mtimeMs, manifest, byUrl: new Map(manifest.pages.map((p) => [p.url, p])), images, views: new Map() };
  return loaded;
}

export function docsManifest(): DocsManifest | null {
  return load()?.manifest ?? null;
}

export type DocsTarget =
  | { kind: "redirect"; location: string }
  | { kind: "page"; page: DocsPageEntry }
  | { kind: "markdown"; page: DocsPageEntry }
  | { kind: "file"; file: string; rel: string }
  | { kind: "missing" };

function safeRelative(pathname: string): string | null {
  let rel: string;
  try {
    rel = decodeURIComponent(pathname.slice(DOCS_BASE.length + 1));
  } catch {
    return null;
  }
  if (rel.includes("\0") || rel.includes("\\")) return null;
  const segments = rel.split("/");
  for (const [i, s] of segments.entries()) {
    if (s === "" && i !== segments.length - 1) return null;
    if (s === "." || s === "..") return null;
    if (s.startsWith(".") && s !== ".gitbook") return null;
  }
  return rel;
}

/** What a /docs request names; `accept` negotiates a page URL to its Markdown. */
export function resolveDocsRequest(url: URL, request?: Request): DocsTarget {
  const state = load();
  const { pathname } = url;
  if (pathname === DOCS_BASE) return { kind: "redirect", location: `${DOCS_BASE}/${url.search}` };
  if (!state || !pathname.startsWith(`${DOCS_BASE}/`)) return { kind: "missing" };
  const rel = safeRelative(pathname);
  if (rel === null) return { kind: "missing" };
  if (rel === "" || rel.endsWith("/")) {
    const page = state.byUrl.get(`${DOCS_BASE}/${rel}`);
    if (!page) return { kind: "missing" };
    return request && wantsMarkdown(request) ? { kind: "markdown", page } : { kind: "page", page };
  }
  if (/(^|\/)index\.md$/.test(rel)) {
    const page = state.byUrl.get(`${DOCS_BASE}/${rel.slice(0, -"index.md".length)}`);
    return page ? { kind: "markdown", page } : { kind: "missing" };
  }
  const last = rel.slice(rel.lastIndexOf("/") + 1);
  if (last.includes(".")) {
    const file = path.join(state.dir, rel);
    try {
      if (fs.statSync(file).isFile()) return { kind: "file", file, rel };
    } catch {}
    return { kind: "missing" };
  }
  if (state.byUrl.has(`${DOCS_BASE}/${rel}/`)) return { kind: "redirect", location: `${DOCS_BASE}/${rel}/${url.search}` };
  return { kind: "missing" };
}

const TYPES: Record<string, string> = {
  png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", gif: "image/gif", webp: "image/webp", avif: "image/avif",
  svg: "image/svg+xml", ico: "image/x-icon", mp4: "video/mp4", webm: "video/webm", mov: "video/quicktime",
  mp3: "audio/mpeg", wav: "audio/wav", ogg: "audio/ogg", aac: "audio/aac", pdf: "application/pdf", zip: "application/zip",
  json: "application/json; charset=utf-8", txt: "text/plain; charset=utf-8", csv: "text/csv; charset=utf-8",
  xml: "application/xml", md: "text/markdown; charset=utf-8", wasm: "application/wasm", glb: "model/gltf-binary",
  gltf: "model/gltf+json", woff: "font/woff", woff2: "font/woff2", ttf: "font/ttf", otf: "font/otf",
};

export function docsContentType(file: string): string {
  return TYPES[path.extname(file).slice(1).toLowerCase()] ?? "application/octet-stream";
}

const LFS_POINTER = "version https://git-lfs.github.com/spec/";

function isLfsPointer(file: string, size: number): boolean {
  if (size > 200 || path.extname(file) === ".md" || path.extname(file) === ".json") return false;
  return fs.readFileSync(file, "latin1").startsWith(LFS_POINTER);
}

export function docsFileResponse(target: Extract<DocsTarget, { kind: "file" }>, method = "GET"): Response {
  const size = fs.statSync(target.file).size;
  if (isLfsPointer(target.file, size)) {
    return new Response("docs media is a Git LFS pointer here: run `git lfs pull` or set DOCS_CONTENT_DIR\n", {
      status: 503,
      headers: { "Content-Type": "text/plain; charset=utf-8", "Cache-Control": "no-store" },
    });
  }
  const immutable = target.rel.startsWith("_remote/");
  const headers = new Headers({
    "Content-Type": docsContentType(target.file),
    "Content-Length": String(size),
    "Cache-Control": immutable ? "public, max-age=31536000, immutable" : "public, max-age=3600",
    "X-Content-Type-Options": "nosniff",
  });
  if (method === "HEAD") return new Response(null, { status: 200, headers });
  const body = Readable.toWeb(fs.createReadStream(target.file)) as unknown as ReadableStream<Uint8Array>;
  return new Response(body, { status: 200, headers });
}

export function docsMarkdownResponse(page: DocsPageEntry): Response {
  const state = load();
  const md = state ? fs.readFileSync(path.join(state.dir, page.file), "utf8") : "";
  const response = markdownResponse(md, { cacheControl: "public, max-age=300" });
  if (page.tokens !== undefined) response.headers.set("x-markdown-tokens", String(page.tokens));
  return response;
}

function flatten(node: DocsNavNode): DocsNavNode[] {
  const out: DocsNavNode[] = [];
  for (const child of node.children) {
    if (child.type !== "section" && child.url) out.push(child);
    out.push(...flatten(child));
  }
  return out;
}

function navPath(node: DocsNavNode, url: string, trail: DocsNavNode[] = []): DocsNavNode[] | null {
  for (const child of node.children) {
    const next = [...trail, child];
    if (child.url === url) return next;
    const nested = navPath(child, url, next);
    if (nested) return nested;
  }
  return null;
}

const link = (node: DocsNavNode | undefined): DocsLink | null => (node?.url ? { title: node.title ?? "", url: node.url } : null);

/** A page with its space's navigation, breadcrumb, neighbours and rendered body (cached per manifest). */
export function docsPageView(page: DocsPageEntry): DocsPageView {
  const state = load();
  if (!state) throw new Error("docs content is not installed");
  const cached = state.views.get(page.url);
  if (cached) return cached;
  const { manifest } = state;
  const space = manifest.spaces.find((s) => s.id === page.space) ?? manifest.spaces[0];
  const nav = manifest.nav[space.id] ?? { children: [] };
  const navUrl = page.navUrl ?? page.url;
  const flat = flatten(nav);
  const index = flat.findIndex((node) => node.url === navUrl);
  const home = space.id === "player" ? `${DOCS_BASE}/` : `${DOCS_BASE}/${space.id}/`;
  const trail = (navPath(nav, navUrl) ?? []).slice(0, -1);
  const markdown = fs.readFileSync(path.join(state.dir, page.file), "utf8");
  const images = (src: string) => state.images[src] ?? state.images[safeDecode(src)];
  const { html, toc } = renderDocsPage({ markdown, description: page.description, icon: page.icon, images });
  const view: DocsPageView = {
    url: page.url,
    title: page.title,
    description: page.description,
    mdUrl: `${page.url}index.md`,
    editUrl: page.source ? `https://github.com/decentraland/docs/edit/main/${page.source.split("/").map(encodeURIComponent).join("/")}` : null,
    updatedAt: page.updatedAt,
    space,
    spaces: manifest.spaces.filter((s) => s.header),
    nav,
    navUrl,
    breadcrumb: [{ title: space.title, url: home }, ...trail.map((n) => ({ title: n.title ?? "", url: n.url ?? null }))],
    prev: index > 0 ? link(flat[index - 1]) : null,
    next: index >= 0 ? link(flat[index + 1]) : null,
    html,
    toc,
  };
  state.views.set(page.url, view);
  return view;
}

function safeDecode(s: string): string {
  try {
    return decodeURI(s);
  } catch {
    return s;
  }
}
