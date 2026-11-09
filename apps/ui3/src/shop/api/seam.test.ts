// The Shop's read modules take their host as arguments: the client, its base, transport and signer, the peers, and
// the creator-sales switch. This holds them to that: none reaches the browser host's wiring (api/host.ts, or the api
// barrel that re-exports it), the page's switches (discovery/flags, lib/remoteFlags, discovery/reads), ui3's catalyst
// or auth modules, or the page itself; nor a component or the purchase's encoding. It checks each module's own
// imports, not its whole closure.
import { readFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { describe, expect, it } from "vitest";

const SRC = join(__dirname, "..", "..");
// The read modules still in ui3. Those already moved (the client, the catalogue, the item, credits, the flags file)
// are held by library/shop/boundary.test.ts, which compiles them without a host at all.
const SEAM = ["shop/checkout/lib/api.ts", "shop/checkout/lib/trade.ts"];

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

const FORBIDDEN =
  /^(shop\/api\/(host|index)|shop\/discovery\/(flags|reads|lib\/remoteFlags)|shop\/discovery\/components\/.*|shop\/checkout\/purchase\/.*|data\/(auth|catalyst)(\/.*)?)$/;

function hostImports(file: string, source: string): string[] {
  return specifiers(source).filter((s) => FORBIDDEN.test(target(file, s) ?? ""));
}

describe("the host seam of the Shop's read modules", () => {
  it.each(SEAM)("%s imports no host wiring, component or purchase encoding, and reads no page", (file) => {
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
    expect(hostImports("shop/discovery/api.ts", 'import { shopFlags } from "./flags";')).toHaveLength(1);
    expect(hostImports("shop/discovery/lib/categories.ts", 'import type { IconName } from "../components/Icon";')).toHaveLength(1);
    expect(hostImports("shop/checkout/lib/api.ts", 'import { ASSET_TYPE } from "../purchase/encoding";')).toHaveLength(1);
    expect(hostImports(file, 'import { SHOP_API_PREFIX } from "./endpoints";')).toEqual([]);
  });
});
