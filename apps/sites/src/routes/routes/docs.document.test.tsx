// @vitest-environment node
import fs from "node:fs";
import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

import { testRequestHandler } from "@/core/lib/server-build.test-helper";
import { writeDocsFixture } from "@/data/lib/docs/fixture.test-helper";
import * as entry from "../entry.server";
import * as root from "../root";
import * as docs from "./docs.$";

const WALLET = "0x00000000000000000000000000000000000000aa";
const COOKIE = `dcl_wallet=${WALLET}; sid=visitor-sid`;

let dir = "";
beforeAll(() => {
  dir = writeDocsFixture();
  vi.stubEnv("DOCS_CONTENT_DIR", dir);
});
afterAll(() => {
  vi.unstubAllEnvs();
  fs.rmSync(dir, { recursive: true, force: true });
});

/**
 * The pipeline `react-router-serve` runs (`createRequestHandler`) over the real entry, root and docs
 * modules. Unmarked, the same docs page is mounted without its middleware, as a private page.
 */
function app(marked: boolean) {
  const page = marked ? docs : { ...docs, middleware: undefined, headers: () => ({ "Cache-Control": "private, no-store" }) };
  return testRequestHandler(entry, [
    { id: "root", path: "", module: root },
    { id: "routes/docs.$", parentId: "root", path: "docs/*", module: page },
  ]);
}

const visit = (marked: boolean, path: string) =>
  app(marked)(new Request(`https://dcl.test${path}`, { headers: { cookie: COOKIE, accept: "text/html" } }));

describe("when a signed-in visitor opens a docs document", () => {
  let response: Response;
  let html: string;

  beforeEach(async () => {
    response = await visit(true, "/docs/player/faq/");
    html = await response.text();
  });

  it("should answer a shared-cacheable page", () => {
    expect(response.status).toBe(200);
    expect(response.headers.get("Cache-Control")).toBe("public, max-age=300");
    expect(html).toContain("<html");
  });

  it("should carry the wallet nowhere in the HTML or its root data", () => {
    expect(html).not.toContain(WALLET);
  });

  it("should render the nav signed out", () => {
    expect(html).toContain("Sign in");
  });
});

describe("when a signed-in visitor's client reads the root data of a docs page", () => {
  let response: Response;

  beforeEach(async () => {
    response = await visit(true, "/docs/player/faq/_.data?_routes=root");
  });

  it("should carry the wallet nowhere in the shared-cacheable reply", async () => {
    expect(response.status).toBe(200);
    expect(response.headers.get("Cache-Control")).toBe("public, max-age=300");
    expect(await response.text()).not.toContain(WALLET);
  });
});

describe("when a signed-in visitor opens the same page as a private document", () => {
  let html: string;

  beforeEach(async () => {
    html = await (await visit(false, "/docs/player/faq/")).text();
  });

  it("should keep the wallet in the HTML", () => {
    expect(html).toContain(WALLET);
  });

  it("should not render the nav signed out", () => {
    expect(html).not.toContain("Sign in");
  });
});
