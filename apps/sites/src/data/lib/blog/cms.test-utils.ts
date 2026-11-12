// A synthetic cms-server for tests: the response shapes of GET /blog/{posts,categories,authors} and
// /assets/{id}, with made-up content. Install it with `installFakeCms()`.
import { vi } from "vitest";

import { resetBlogCms } from "./cms";
import type { CmsEntry } from "./normalize";

export const CMS_BASE = "https://cms.example/spaces/space-1/environments/master";

const link = (linkType: "Entry" | "Asset", id: string) => ({ sys: { id, type: "Link", linkType } });

const asset = (id: string, title: string, width = 1200, height = 600): CmsEntry => ({
  sys: { id },
  fields: { title, file: { url: `//images.example/${id}.png`, contentType: "image/png", details: { image: { width, height } } } },
});

export const category = (id: string, slug: string, title: string, isShownInMenu = true): CmsEntry => ({
  sys: { id },
  fields: { id: slug, title, description: `About ${title}`, isShownInMenu, image: link("Asset", "cat-image") },
});

const textNode = (value: string, marks: string[] = []) => ({ nodeType: "text", value, marks: marks.map((type) => ({ type })), data: {} });

export const FIRST_POST_BODY = {
  nodeType: "document",
  data: {},
  content: [
    { nodeType: "paragraph", data: {}, content: [textNode("Opening "), textNode("bold", ["bold"]), textNode(" words.")] },
    { nodeType: "heading-2", data: {}, content: [textNode("A section")] },
    {
      nodeType: "paragraph",
      data: {},
      content: [
        textNode("Read "),
        { nodeType: "hyperlink", data: { uri: "https://example.org/docs" }, content: [textNode("the docs")] },
        textNode(" or "),
        { nodeType: "hyperlink", data: { uri: "javascript:alert(1)" }, content: [textNode("this")] },
        textNode("."),
      ],
    },
    { nodeType: "embedded-asset-block", data: { target: link("Asset", "body-image") }, content: [] },
    { nodeType: "unordered-list", data: {}, content: [{ nodeType: "list-item", data: {}, content: [{ nodeType: "paragraph", data: {}, content: [textNode("One item")] }] }] },
  ],
};

type PostSeed = { id: string; slug: string; title: string; category: string; author?: string; image?: string; date: string; body?: unknown };

export const post = (seed: PostSeed): CmsEntry => ({
  sys: { id: seed.id },
  fields: {
    id: seed.slug,
    title: seed.title,
    description: `${seed.title} in short`,
    publishedDate: seed.date,
    category: link("Entry", seed.category),
    ...(seed.author ? { author: link("Entry", seed.author) } : {}),
    ...(seed.image ? { image: link("Asset", seed.image) } : {}),
    body: seed.body ?? { nodeType: "document", data: {}, content: [{ nodeType: "paragraph", data: {}, content: [textNode(`${seed.title} body.`)] }] },
  },
});

export const CMS_CATEGORIES: CmsEntry[] = [
  category("cat-platform", "platform", "Platform"),
  category("cat-news", "announcements", "Announcements"),
  category("cat-hidden", "interview", "Interview", false),
];

export const CMS_AUTHORS: CmsEntry[] = [
  { sys: { id: "author-jane" }, fields: { id: "jane-doe", title: "Jane Doe", description: "Writes about the platform.", image: link("Asset", "jane-face") } },
  { sys: { id: "author-sam" }, fields: { title: "Sam &amp; Co" } },
];

export const CMS_ASSETS: CmsEntry[] = [
  asset("hero-1", "A plaza at night"),
  asset("body-image", "The new map", 800, 400),
  asset("jane-face", "Jane", 200, 200),
  asset("cat-image", "Category"),
];

/** Newest first: the first post (with a picture), a second in the same category without one, then 9 announcements. */
export const CMS_POSTS: CmsEntry[] = [
  post({ id: "p1", slug: "first-post", title: "First Post", category: "cat-platform", author: "author-jane", image: "hero-1", date: "2026-07-02T10:00-07:00", body: FIRST_POST_BODY }),
  post({ id: "p2", slug: "second-post", title: "Second Post", category: "cat-platform", author: "author-sam", date: "2026-07-01T09:00:00Z" }),
  ...Array.from({ length: 9 }, (_, i) =>
    post({ id: `n${i}`, slug: `news-${i + 1}`, title: `News ${i + 1}`, category: "cat-news", date: `2026-06-${String(20 - i).padStart(2, "0")}T12:00:00Z` }),
  ),
];

export type FakeCmsFailure = "status" | "json" | "network" | "hang";

/** What a test changes in the fake CMS: its content (read on every request) and which requests hang. */
export type FakeCmsOptions = {
  posts?: () => CmsEntry[];
  categories?: () => CmsEntry[];
  hang?: (url: URL) => boolean;
  fail?: (url: URL) => Response | undefined;
};

const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });

const slugOf = (entries: CmsEntry[], id: string) => entries.find((e) => e.sys.id === id)?.fields?.id;

/** Answers a cms-server request, or fails it as `fail` says. */
export function fakeCmsFetch(fail?: FakeCmsFailure, options: FakeCmsOptions = {}) {
  const posts = options.posts ?? (() => CMS_POSTS);
  const categories = options.categories ?? (() => CMS_CATEGORIES);
  return vi.fn(async (input: string | URL | Request, init?: RequestInit): Promise<Response> => {
    const url = new URL(typeof input === "string" ? input : input instanceof URL ? input.href : input.url);
    if (fail === "network") throw new TypeError("fetch failed");
    if (fail === "hang" || options.hang?.(url))
      return new Promise((_, reject) => init?.signal?.addEventListener("abort", () => reject(init.signal!.reason), { once: true }));
    const failed = options.fail?.(url);
    if (failed) return failed;
    if (fail === "status") return new Response("upstream down", { status: 502 });
    if (fail === "json") return new Response("<html>not json</html>", { status: 200 });
    if (!url.href.startsWith(`${CMS_BASE}/`)) return new Response("unknown", { status: 404 });
    const path = url.href.slice(CMS_BASE.length).split("?")[0];
    const p = url.searchParams;
    const limit = Number(p.get("limit") ?? 20);
    const skip = Number(p.get("skip") ?? 0);
    const page = (items: CmsEntry[]) => json({ items: items.slice(skip, skip + limit), total: items.length, skip, limit });
    if (path === "/blog/categories") return page(categories());
    if (path === "/blog/authors") return page(CMS_AUTHORS);
    if (path === "/blog/posts") {
      const q = p.get("q")?.toLowerCase();
      const items = posts().filter((e) => {
        const f = e.fields ?? {};
        if (p.get("slug") && f.id !== p.get("slug")) return false;
        if (p.get("category") && slugOf(categories(), (f.category as { sys: { id: string } }).sys.id) !== p.get("category")) return false;
        if (p.get("author")) {
          const authorId = (f.author as { sys: { id: string } } | undefined)?.sys.id ?? "";
          const author = CMS_AUTHORS.find((a) => a.sys.id === authorId);
          if (!author || (author.fields?.id ?? author.sys.id) !== p.get("author")) return false;
        }
        return !q || String(f.title).toLowerCase().includes(q);
      });
      return page(items);
    }
    const assetMatch = /^\/assets\/([^/]+)$/.exec(path);
    if (assetMatch) {
      const found = CMS_ASSETS.find((a) => a.sys.id === decodeURIComponent(assetMatch[1]));
      return found ? json(found) : json({ message: "Not found" }, 404);
    }
    return json({ message: "Not found" }, 404);
  });
}

/** Points BLOG_CMS_URL at the fake CMS and stubs fetch; returns the fetch mock. Clears the CMS memos. */
export function installFakeCms(fail?: FakeCmsFailure, options?: FakeCmsOptions) {
  resetBlogCms();
  vi.stubEnv("BLOG_CMS_URL", CMS_BASE);
  const fetchMock = fakeCmsFetch(fail, options);
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

/** The URLs the fake CMS was asked for, in order. */
export const requestedUrls = (fetchMock: ReturnType<typeof fakeCmsFetch>) =>
  fetchMock.mock.calls.map(([input]) => (typeof input === "string" ? input : input instanceof URL ? input.href : input.url));
