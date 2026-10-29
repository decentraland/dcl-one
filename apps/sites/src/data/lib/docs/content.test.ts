import fs from "node:fs";
import path from "node:path";
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";

import { docsPageView, resolveDocsRequest } from "./content.server";
import { writeDocsFixture } from "./fixture.test-helper";
import { renderDocsPage } from "./render";

let dir = "";
beforeAll(() => { dir = writeDocsFixture(); vi.stubEnv("DOCS_CONTENT_DIR", dir); });
afterAll(() => { vi.unstubAllEnvs(); fs.rmSync(dir, { recursive: true, force: true }); });

const resolve = (p: string, accept?: string) =>
  resolveDocsRequest(new URL(`https://dcl.test${p}`), new Request(`https://dcl.test${p}`, { headers: accept ? { accept } : {} }));

describe("resolveDocsRequest", () => {
  it("maps page URLs, the Player alias and Markdown alternates", () => {
    expect(resolve("/docs/")).toMatchObject({ kind: "page", page: { url: "/docs/", navUrl: "/docs/player/" } });
    expect(resolve("/docs/player/")).toMatchObject({ kind: "page", page: { url: "/docs/player/" } });
    expect(resolve("/docs/player/faq/index.md")).toMatchObject({ kind: "markdown", page: { url: "/docs/player/faq/" } });
    expect(resolve("/docs/index.md")).toMatchObject({ kind: "markdown", page: { url: "/docs/" } });
    expect(resolve("/docs/player/faq/", "text/markdown")).toMatchObject({ kind: "markdown" });
  });

  it("redirects the bare prefix and extensionless directories", () => {
    expect(resolve("/docs")).toEqual({ kind: "redirect", location: "/docs/" });
    expect(resolve("/docs/player/faq")).toEqual({ kind: "redirect", location: "/docs/player/faq/" });
  });

  it("serves committed files and refuses traversal and dotfiles", () => {
    expect(resolve("/docs/.gitbook/assets/shot.webp")).toMatchObject({ kind: "file", rel: ".gitbook/assets/shot.webp" });
    expect(resolve("/docs/nope/")).toEqual({ kind: "missing" });
    expect(resolve("/docs/%2e%2e/manifest.json")).toEqual({ kind: "missing" });
    expect(resolve("/docs/.git/config")).toEqual({ kind: "missing" });
    expect(resolve("/docs/missing.png")).toEqual({ kind: "missing" });
  });
});

describe("docsPageView", () => {
  it("builds navigation, breadcrumb, neighbours and sized images", () => {
    const target = resolve("/docs/creator/intro/deep/");
    if (target.kind !== "page") throw new Error("expected a page");
    const deep = docsPageView(target.page);
    expect(deep.breadcrumb).toEqual([{ title: "Creator", url: "/docs/creator/" }, { title: "Scenes", url: null }, { title: "Intro", url: "/docs/creator/intro/" }]);
    expect(deep.prev).toEqual({ title: "Intro", url: "/docs/creator/intro/" });
    expect(deep.next).toBeNull();
    expect(deep.editUrl).toBe("https://github.com/decentraland/docs/edit/main/creator/intro/deep.md");
    const intro = resolve("/docs/creator/intro/");
    if (intro.kind !== "page") throw new Error("expected a page");
    expect(docsPageView(intro.page).html).toContain('<img loading="lazy" decoding="async" src="/docs/.gitbook/assets/shot.webp" alt="shot" width="1600" height="800">');
  });

  it("keeps percentage-sized images as authored", async () => {
    const { addImageDimensions } = await import("./images");
    const entry = { source: "x.png", width: 1170, height: 1710, bytes: 1 };
    expect(addImageDimensions('<img src="/docs/x.webp" width="33%">', () => entry)).toBe('<img src="/docs/x.webp" width="33%">');
    expect(addImageDimensions('<img src="/docs/x.webp" width="585">', () => entry)).toBe('<img src="/docs/x.webp" width="585" height="855">');
  });

  it("gives the home page the Player navigation entry", () => {
    const home = resolve("/docs/");
    if (home.kind !== "page") throw new Error("expected a page");
    const view = docsPageView(home.page);
    expect(view.navUrl).toBe("/docs/player/");
    expect(view.next).toEqual({ title: "FAQ", url: "/docs/player/faq/" });
    expect(view.mdUrl).toBe("/docs/index.md");
  });
});

describe("renderDocsPage", () => {
  it("turns alerts into hints, ids headings and moves the description into the lead", () => {
    const md = fs.readFileSync(path.join(dir, "index.md"), "utf8");
    const { html, toc } = renderDocsPage({ markdown: md, description: "Start here", icon: "hand-wave" });
    expect(html).toContain('<h2 id="first-steps"');
    expect(html).toMatch(/class="hint hint-warning"[\s\S]*Mind the gap/);
    expect(html).toContain('<p class="lead">Start here</p>');
    expect(html).not.toContain("<blockquote>\n<p>Start here");
    expect(toc).toEqual([{ depth: 2, id: "first-steps", text: "First steps" }]);
  });
});
