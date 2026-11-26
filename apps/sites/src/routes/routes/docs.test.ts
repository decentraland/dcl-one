import { RouterContextProvider } from "react-router";
import fs from "node:fs";
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";

import { pageSid, sessionMiddleware } from "@/core/lib/telemetry/session";
import { writeDocsFixture } from "@/data/lib/docs/fixture.test-helper";
import { loader, middleware } from "./docs.$";

let dir = "";
beforeAll(() => { dir = writeDocsFixture(); vi.stubEnv("DOCS_CONTENT_DIR", dir); });
afterAll(() => { vi.unstubAllEnvs(); fs.rmSync(dir, { recursive: true, force: true }); });

const request = (p: string, init?: RequestInit) => new Request(`https://dcl.test${p}`, init);
const next = vi.fn(async () => new Response("rendered"));
async function run(p: string, init?: RequestInit): Promise<Response> {
  next.mockClear();
  const result = await middleware[0]({ request: request(p, init), context: new RouterContextProvider() } as never, next as never);
  return result instanceof Response ? result : new Response("fallthrough");
}
async function load(p: string) {
  try {
    return await loader({ request: request(p), context: new RouterContextProvider() } as never);
  } catch (thrown) {
    return thrown;
  }
}

describe("/docs middleware", () => {
  it("renders pages through the loader", async () => {
    expect(await (await run("/docs/player/faq/")).text()).toBe("rendered");
    expect(next).toHaveBeenCalledOnce();
  });

  it("answers the .md alternate with Markdown and its token header", async () => {
    const response = await run("/docs/player/faq/index.md");
    expect(response.headers.get("content-type")).toMatch(/^text\/markdown/);
    expect(response.headers.get("x-markdown-tokens")).toBe("42");
    expect(await response.text()).toContain("## Why?");
    const negotiated = await run("/docs/", { headers: { accept: "text/markdown" } });
    expect(await negotiated.text()).toContain("# Welcome");
    expect(Number(negotiated.headers.get("x-markdown-tokens"))).toBeGreaterThan(0);
  });

  it("serves assets with their type and 404s missing ones", async () => {
    const image = await run("/docs/.gitbook/assets/shot.webp");
    expect(image.headers.get("content-type")).toBe("image/webp");
    expect(new TextDecoder().decode(new Uint8Array(await image.arrayBuffer()).slice(0, 4))).toBe("RIFF");
    expect((await run("/docs/.gitbook/assets/shot.png")).status).toBe(404);
    expect((await run("/docs/.gitbook/assets/shot.webp", { method: "POST" })).status).toBe(405);
    fs.writeFileSync(`${dir}/.gitbook/assets/pointer.png`, "version https://git-lfs.github.com/spec/v1\noid sha256:00\nsize 1\n");
    expect((await run("/docs/.gitbook/assets/pointer.png")).status).toBe(503);
  });

  it("redirects /docs and extensionless page URLs", async () => {
    const bare = await run("/docs");
    expect([bare.status, bare.headers.get("location")]).toEqual([301, "/docs/"]);
    expect((await run("/docs/player/faq")).headers.get("location")).toBe("/docs/player/faq/");
  });
});

describe("/docs loader", () => {
  it("loads a page view", async () => {
    expect(await load("/docs/player/faq/")).toMatchObject({ title: "FAQ", mdUrl: "/docs/player/faq/index.md", toc: [{ id: "why" }, { id: "how", depth: 3 }] });
  });

  it("keeps /docs/player/ as an alias of the home page", async () => {
    const home = await load("/docs/"), alias = await load("/docs/player/");
    expect(home).toMatchObject({ url: "/docs/", navUrl: "/docs/player/", title: "Welcome" });
    expect(alias).toMatchObject({ url: "/docs/player/", navUrl: "/docs/player/", html: (home as { html: string }).html });
  });

  it("404s unknown pages", async () => {
    expect(await load("/docs/nope/")).toMatchObject({ init: { status: 404 } });
  });
});

describe("/docs and the visitor session", () => {
  // The real docs middleware under the root session middleware. The rendered stub carries no
  // Cache-Control, so only the docs route marking itself public keeps the session off the reply.
  async function visit(p: string) {
    const context = new RouterContextProvider();
    const req = request(p);
    let rootSid: string | null | undefined;
    const response = await sessionMiddleware(
      { request: req, context } as never,
      (async () => {
        const answered = await middleware[0]({ request: req, context } as never, (async () => {
          rootSid = pageSid(req, context);
          return new Response("rendered");
        }) as never);
        return answered instanceof Response ? answered : new Response("fallthrough");
      }) as never,
    );
    return { response: response as Response, rootSid };
  }

  it.each(["/docs/player/faq/", "/docs/player/faq.data"])("gives %s no sid cookie and no sid in root data", async (p) => {
    const { response, rootSid } = await visit(p);

    expect(await response.text()).toBe("rendered");
    expect(rootSid).toBeNull();
    expect(response.headers.getSetCookie()).toEqual([]);
  });

  it("gives a docs file no sid cookie", async () => {
    const { response } = await visit("/docs/.gitbook/assets/shot.webp");

    expect(response.status).toBe(200);
    expect(response.headers.getSetCookie()).toEqual([]);
  });
});
