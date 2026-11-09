// The Shop's read modules take their host as arguments: the client, its base, transport and signer, and the
// peers. This holds them to that: none reaches the browser host's wiring (api/host.ts, or the api barrel that
// re-exports it), ui3's catalyst or auth modules, or the page itself. It checks each module's own imports, not its
// whole closure: discovery/api still imports modules that are not portable yet.
import { readFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { describe, expect, it } from "vitest";

const SRC = join(__dirname, "..", "..");
const SEAM = [
  "shop/api/client.ts",
  "shop/api/shop.ts",
  "shop/api/signer.ts",
  "shop/checkout/lib/api.ts",
  "shop/credits/credits.ts",
  "shop/detail/api.ts",
  "shop/detail/lib/urn.ts",
  "shop/discovery/api.ts",
];

// The code only: a comment may name what the module no longer reaches for.
const code = (source: string): string => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/.*$/gm, "");

/** Every module a file names: static imports and re-exports (side-effect ones included) and dynamic imports. */
function specifiers(source: string): string[] {
  return [
    ...source.matchAll(/^\s*(?:import|export)\s+(?:[^'";]*?\sfrom\s+)?['"]([^'"]+)['"]/gm),
    ...source.matchAll(/\bimport\(\s*['"]([^'"]+)['"]\s*\)/g),
  ].map((m) => m[1] as string);
}

/** Where a relative specifier lands, from src/ and without its extension; the api folder lands on its index. */
function target(file: string, specifier: string): string | null {
  if (!specifier.startsWith(".")) return null;
  const path = relative(SRC, resolve(dirname(join(SRC, file)), specifier)).replace(/\.(ts|tsx)$/, "");
  return path === "shop/api" ? "shop/api/index" : path;
}

const FORBIDDEN = /^(shop\/api\/(host|index)|data\/(auth|catalyst)(\/.*)?)$/;

function hostImports(file: string, source: string): string[] {
  return specifiers(source).filter((s) => FORBIDDEN.test(target(file, s) ?? ""));
}

describe("the host seam of the Shop's read modules", () => {
  it.each(SEAM)("%s imports no host wiring and reads no page", (file) => {
    const source = code(readFileSync(join(SRC, file), "utf8"));
    expect(hostImports(file, source)).toEqual([]);
    expect(source).not.toMatch(/\b(window|document|localStorage|sessionStorage|location)\b|import\.meta|\bfetch\(/);
    expect(source).not.toMatch(/\bshopClient\(\)/);
  });

  it("finds the host however a module names it", () => {
    const file = "shop/api/client.ts";
    for (const line of [
      'import { hostApiOptions } from "./host";',
      'import { hostApiOptions } from "./index";',
      'import { hostApiOptions } from ".";',
      'export { shopClient } from "./host.ts";',
      'const host = await import("./host");',
      'import "../../data/auth/identity";',
      'import type { QueryParams } from "../../data/catalyst/client";',
    ]) {
      expect(hostImports(file, line), line).toHaveLength(1);
    }
    expect(hostImports("shop/discovery/api.ts", 'import { shopClient } from "../api";')).toHaveLength(1);
    expect(hostImports(file, 'import { SHOP_API_PREFIX } from "./endpoints";')).toEqual([]);
  });
});
