// The Shop's read modules take their host as arguments: the client, its base, transport and signer, the peers, the
// creator-sales switch, and the Ethereum network with its reader. This holds them to that: none reaches the browser
// host's wiring (api/host.ts, or the api barrel that re-exports it), the page's switches (discovery/flags,
// lib/remoteFlags, discovery/reads), the host's network and readers (land/chain, land/evm, checkout/chain, wallet),
// the NAME registration or its host reads (names/names, names/offer, names/reads), ui3's catalyst or auth modules, or
// the page itself; nor a component, the purchase's encoding or the wallet's MANA. It checks each module's own imports,
// not its whole closure.
import { readFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { describe, expect, it } from "vitest";

const SRC = join(__dirname, "..", "..");
// The read modules still in ui3. Those already moved (the client, the catalogue, the item, credits, the flags file)
// are held by library/shop/boundary.test.ts, which compiles them without a host at all.
const SEAM = [
  "shop/checkout/lib/api.ts",
  "shop/checkout/lib/trade.ts",
  "shop/land/contracts.ts",
  "shop/land/read.ts",
  "shop/names/api.ts",
  "shop/names/name.ts",
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
 * are skipped whole, so a `//` or a `/*` inside one hides nothing. A template's `${…}` is code again, read the same way
 * up to the brace that closes it, so a template nested inside one is skipped whole too. A `/` starts a regular
 * expression where an expression may start (after an operator, a bracket, a comma or a keyword), as JavaScript reads it.
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
        out += source[i++];
        out += source[i++];
        last = "{";
        skipCode(true);
        continue;
      }
      out += source[i++] ?? "";
    }
    out += source[i++] ?? "";
  };
  /** Code to the end of the source, or, in a template's `${`, through the brace that closes it. */
  const skipCode = (interpolation: boolean): void => {
    let depth = 0;
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
        i++;
        if (!/\s/.test(c)) last = c;
        if (c === "{") depth++;
        else if (c === "}" && depth-- === 0 && interpolation) return;
      }
    }
  };
  skipCode(false);
  return out;
}

/** Where a relative specifier lands, from src/ and without its extension; the api folder lands on its index. */
function target(file: string, specifier: string): string | null {
  if (!specifier.startsWith(".")) return null;
  const path = relative(SRC, resolve(dirname(join(SRC, file)), specifier)).replace(/\.(ts|tsx|mts|js|mjs|jsx)$/, "");
  return path === "shop/api" ? "shop/api/index" : path;
}

const FORBIDDEN =
  /^(shop\/api\/(host|index)|shop\/discovery\/(flags|reads|lib\/remoteFlags)|shop\/discovery\/components\/.*|shop\/checkout\/(purchase|chain)\/.*|shop\/land\/(chain|evm)|shop\/names\/(names|offer|reads)|shop\/market\/lib\/mana|shop\/wallet\/.*|data\/(auth|catalyst)(\/.*)?)$/;

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
    const file = "shop/checkout/lib/api.ts";
    for (const line of [
      'import { hostApiOptions } from "../../api/host";',
      'import { hostApiOptions } from "../../api/index";',
      'import { hostApiOptions } from "../../api";',
      'export { shopClient } from "../../api/host.ts";',
      'const host = await import("../../api/host");',
      'import "../../../data/auth/identity";',
      'import type { QueryParams } from "../../../data/catalyst/client";',
    ]) {
      expect(hostImports(file, line), line).toHaveLength(1);
    }
    expect(hostImports(file, 'import { shopFlags } from "../../discovery/flags";')).toHaveLength(1);
    expect(hostImports("shop/checkout/lib/trade.ts", 'import type { IconName } from "../../discovery/components/Icon";')).toHaveLength(1);
    expect(hostImports(file, 'import { ASSET_TYPE } from "../purchase/encoding";')).toHaveLength(1);
    expect(hostImports("shop/land/contracts.ts", 'import { ethereumChainId } from "../checkout/chain/config";')).toHaveLength(1);
    expect(hostImports("shop/names/name.ts", 'import { landChainId } from "../land/chain";')).toHaveLength(1);
    expect(hostImports("shop/names/name.ts", 'import { rpcReader } from "../land/evm";')).toHaveLength(1);
    expect(hostImports("shop/names/api.ts", 'import { parseManaInput } from "../market/lib/mana";')).toHaveLength(1);
    expect(hostImports("shop/names/api.ts", 'import { shopClient } from "../api/host";')).toHaveLength(1);
    expect(hostImports("shop/names/api.ts", 'import { readBannedNames } from "./reads";')).toHaveLength(1);
    expect(hostImports("shop/names/name.ts", 'import { requireChain } from "../wallet/chain";')).toHaveLength(1);
    expect(hostImports("shop/names/api.ts", 'import { NAME_PRICE_IN_WEI } from "./name";')).toEqual([]);
    expect(hostImports("shop/names/name.ts", 'import { readContract } from "../land/read";')).toEqual([]);
    expect(hostImports(file, 'import { ASSET_TYPE } from "./trade";')).toEqual([]);
    expect(hostImports(file, 'const host = require("../../api/host");')).toHaveLength(1);
    expect(hostImports(file, 'import { hostApiOptions } from "../../api/host.js";')).toHaveLength(1);
    expect(hostImports(file, '// import { shopClient } from "../../api/host";')).toHaveLength(1);
    expect(hostImports(file, "const host = await import(where);")).toEqual(["import(where)"]);
  });

  it("blanks comments only: a // or /* inside a string, a template or a regular expression hides nothing", () => {
    expect(withoutComments('const base = "http://x"; window.alert(1); // gone')).toBe('const base = "http://x"; window.alert(1); ');
    expect(withoutComments('const open = "/*"; fetch(url); /* gone */ done()')).toBe('const open = "/*"; fetch(url);   done()');
    expect(withoutComments("const t = `a ${'//'} b`; location.href")).toBe("const t = `a ${'//'} b`; location.href");
    expect(withoutComments(`const q = /['"]/; window.x; // gone`)).toBe(`const q = /['"]/; window.x; `);
    expect(withoutComments("const r = a / b; // gone")).toBe("const r = a / b; ");
    // A template nested in a `${…}`, with a URL in it, ends neither its own template nor the outer one.
    expect(withoutComments("const t = `a ${`http://x`} b`; window.location; // gone")).toBe("const t = `a ${`http://x`} b`; window.location; ");
    expect(withoutComments("const t = `${`http://${host}/`}`; window.x")).toBe("const t = `${`http://${host}/`}`; window.x");
    // Inside a `${…}`, braces nest, a comment is still a comment, and a `/` may start a regular expression.
    expect(withoutComments("const t = `${f({ a: 1 })}//`; window.y")).toBe("const t = `${f({ a: 1 })}//`; window.y");
    expect(withoutComments("const t = `${x /* } */}`; window.z")).toBe("const t = `${x  }`; window.z");
    expect(withoutComments("const t = `${/`/.test(s)}`; window.w")).toBe("const t = `${/`/.test(s)}`; window.w");
  });
});
