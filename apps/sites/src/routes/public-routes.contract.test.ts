import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { beforeEach, describe, expect, it } from "vitest";

const ROUTES = fileURLToPath(new URL("./routes", import.meta.url));

const PUBLIC_CACHE = /cache-control["']?\s*[:,]\s*["'`][^"'`]*\b(public|s-maxage)\b/i;
const isDocument = (source: string) => /export\s+default\b|export\s*\{[^}]*\bdefault\b/.test(source);

/**
 * A document route a shared cache may store renders root data too, and the root middleware only drops
 * the cookie: the sid in that data is dropped only when the route calls `markPublicResponse`.
 */
describe("public document routes", () => {
  describe("when reading the route modules", () => {
    let files: string[];

    beforeEach(() => {
      files = fs.readdirSync(ROUTES).filter((f) => /\.tsx?$/.test(f) && !/\.test\.tsx?$/.test(f));
    });

    it("should find the route modules", () => {
      expect(files.length).toBeGreaterThan(50);
    });

    it("should have each public document route mark itself public, so root data carries no sid", () => {
      const unmarked = files.filter((f) => {
        const source = fs.readFileSync(path.join(ROUTES, f), "utf8");
        return isDocument(source) && PUBLIC_CACHE.test(source) && !source.includes("markPublicResponse(");
      });

      expect(unmarked).toEqual([]);
    });
  });

  describe("when a document route sets a public header and does not mark itself", () => {
    let source: string;

    beforeEach(() => {
      source = 'export function headers() { return { "Cache-Control": "public, max-age=60" }; }\nexport default function P() {}';
    });

    it("should detect the unmarked public route", () => {
      expect(isDocument(source) && PUBLIC_CACHE.test(source) && !source.includes("markPublicResponse(")).toBe(true);
    });
  });
});
