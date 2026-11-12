import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const ROUTES = fileURLToPath(new URL("./routes", import.meta.url));

const PUBLIC_CACHE = /cache-control["']?\s*[:,]\s*["'`][^"'`]*\b(public|s-maxage)\b/i;
const isDocument = (source: string) => /export\s+default\b|export\s*\{[^}]*\bdefault\b/.test(source);

/**
 * A document route a shared cache may store renders root data too, and the root middleware only drops
 * the cookie: the sid in that data is dropped only when the route calls `markPublicResponse`.
 */
describe("public document routes", () => {
  const files = fs.readdirSync(ROUTES).filter((f) => /\.tsx?$/.test(f) && !/\.test\.tsx?$/.test(f));

  it("finds the route modules", () => {
    expect(files.length).toBeGreaterThan(50);
  });

  it("each mark themselves public, so root data carries no sid", () => {
    const unmarked = files.filter((f) => {
      const source = fs.readFileSync(path.join(ROUTES, f), "utf8");
      return isDocument(source) && PUBLIC_CACHE.test(source) && !source.includes("markPublicResponse(");
    });

    expect(unmarked).toEqual([]);
  });

  it("detects a public header in a document route that does not mark itself", () => {
    const source = 'export function headers() { return { "Cache-Control": "public, max-age=60" }; }\nexport default function P() {}';

    expect(isDocument(source) && PUBLIC_CACHE.test(source) && !source.includes("markPublicResponse(")).toBe(true);
  });
});
