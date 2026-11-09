// The Shop's read modules take their host as arguments: the client, its base, transport and signer, the peers, and
// the creator-sales switch. This holds them to that: none reaches the browser host's wiring (api/host.ts, or the api
// barrel that re-exports it), the page's switches (discovery/flags, lib/remoteFlags, discovery/reads), ui3's catalyst
// or auth modules, or the page itself; nor a component or the purchase's encoding. It checks each module's own
// imports, not its whole closure.
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
  "shop/discovery/lib/categories.ts",
  "shop/discovery/lib/iconName.ts",
  "shop/discovery/lib/rolloutFlags.ts",
  "shop/checkout/lib/trade.ts",
];

/**
 * Every module a file names, read from its raw source as library/shop/boundary.test.ts reads it, so that a mistake
 * here can only fail: static imports and re-exports (side-effect ones included), dynamic imports and require. A
 * commented-out import counts too. A specifier that is not a literal is opaque, and is reported as such.
 */
function specifiers(source: string): { named: string[]; opaque: string[] } {
  const named = [
    ...source.matchAll(/^\s*(?:\/\/+\s*|\*\s*)?(?:import|export)\s+(?:[^'";]*?\sfrom\s+)?['"]([^'"]+)['"]/gm),
    ...source.matchAll(/\bimport\(\s*['"]([^'"]+)['"]\s*\)/g),
    ...source.matchAll(/\brequire\(\s*['"]([^'"]+)['"]\s*\)/g),
  ].map((m) => m[1] as string);
  const opaque = [
    ...[...source.matchAll(/\bimport\(\s*([^'"\s][^)]*)\)/g)].map((m) => `import(${m[1]})`),
    ...[...source.matchAll(/\brequire\(\s*([^'"\s][^)]*)\)/g)].map((m) => `require(${m[1]})`),
  ];
  return { named, opaque };
}

/**
 * The source with its comments blanked and everything else kept: strings, template literals and regular expressions
 * are skipped whole, so a `//` or a `/*` inside one hides nothing. A `/` starts a regular expression where an
 * expression may start (after an operator, a bracket, a comma or a keyword), as JavaScript reads it.
 */
function withoutComments(source: string): string {
  let out = "";
  let i = 0;
  let last = ""; // the last significant character outside comments and literals
  const skipQuoted = (quote: string): void => {
    out += source[i++];
    while (i < source.length && source[i] !== quote) {
      if (source[i] === "\\") out += source[i++];
      out += source[i++] ?? "";
    }
    out += source[i++] ?? "";
  };
  const skipTemplate = (): void => {
    out += source[i++];
    while (i < source.length && source[i] !== "`") {
      if (source[i] === "\\") out += source[i++];
      else if (source[i] === "$" && source[i + 1] === "{") {
        let depth = 0;
        do {
          if (source[i] === "{") depth++;
          else if (source[i] === "}") depth--;
          else if (source[i] === "'" || source[i] === '"') {
            skipQuoted(source[i]!);
            continue;
          }
          out += source[i++];
        } while (i < source.length && depth > 0);
        continue;
      }
      out += source[i++] ?? "";
    }
    out += source[i++] ?? "";
  };
  while (i < source.length) {
    const c = source[i]!;
    const next = source[i + 1];
    if (c === "/" && next === "/") {
      while (i < source.length && source[i] !== "\n") i++;
    } else if (c === "/" && next === "*") {
      const end = source.indexOf("*/", i + 2);
      i = end < 0 ? source.length : end + 2;
      out += " ";
    } else if (c === "'" || c === '"') {
      skipQuoted(c);
      last = c;
    } else if (c === "`") {
      skipTemplate();
      last = c;
    } else if (c === "/" && (last === "" || /[(,=:[!&|?{};+\-*%<>~^]/.test(last) || /\b(return|typeof|case|in|of)\s*$/.test(out))) {
      out += source[i++];
      let inClass = false;
      while (i < source.length && (source[i] !== "/" || inClass)) {
        if (source[i] === "\\") out += source[i++];
        else if (source[i] === "[") inClass = true;
        else if (source[i] === "]") inClass = false;
        out += source[i++] ?? "";
      }
      out += source[i++] ?? "";
      last = "/";
    } else {
      out += c;
      if (!/\s/.test(c)) last = c;
      i++;
    }
  }
  return out;
}

/** Where a relative specifier lands, from src/ and without its extension; the api folder lands on its index. */
function target(file: string, specifier: string): string | null {
  if (!specifier.startsWith(".")) return null;
  const path = relative(SRC, resolve(dirname(join(SRC, file)), specifier)).replace(/\.(ts|tsx|mts|js|mjs|jsx)$/, "");
  return path === "shop/api" ? "shop/api/index" : path;
}

const FORBIDDEN =
  /^(shop\/api\/(host|index)|shop\/discovery\/(flags|reads|lib\/remoteFlags)|shop\/discovery\/components\/.*|shop\/checkout\/purchase\/.*|data\/(auth|catalyst)(\/.*)?)$/;

function hostImports(file: string, source: string): string[] {
  const { named, opaque } = specifiers(source);
  return [...named.filter((s) => FORBIDDEN.test(target(file, s) ?? "")), ...opaque];
}

describe("the host seam of the Shop's read modules", () => {
  it.each(SEAM)("%s imports no host wiring, component or purchase encoding, and reads no page", (file) => {
    const raw = readFileSync(join(SRC, file), "utf8");
    expect(hostImports(file, raw)).toEqual([]);
    const source = withoutComments(raw);
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
    expect(hostImports(file, 'const host = require("./host");')).toHaveLength(1);
    expect(hostImports(file, 'import { hostApiOptions } from "./host.js";')).toHaveLength(1);
    expect(hostImports(file, '// import { shopClient } from "./host";')).toHaveLength(1);
    expect(hostImports(file, "const host = await import(where);")).toEqual(["import(where)"]);
  });

  it("blanks comments only: a // or /* inside a string, a template or a regular expression hides nothing", () => {
    expect(withoutComments('const base = "http://x"; window.alert(1); // gone')).toBe('const base = "http://x"; window.alert(1); ');
    expect(withoutComments('const open = "/*"; fetch(url); /* gone */ done()')).toBe('const open = "/*"; fetch(url);   done()');
    expect(withoutComments("const t = `a ${'//'} b`; location.href")).toBe("const t = `a ${'//'} b`; location.href");
    expect(withoutComments(`const q = /['"]/; window.x; // gone`)).toBe(`const q = /['"]/; window.x; `);
    expect(withoutComments("const r = a / b; // gone")).toBe("const r = a / b; ");
  });
});
