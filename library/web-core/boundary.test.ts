// library/web-core holds the non-UI browser logic Sites and ui3 share. What this file holds it to:
//
// - TypeScript modules, their tests, the seam's build alias (validate/vite.mjs) and its own config;
// - its modules import only the library itself, the seam alias (`dcl-validate-impl`) and the packages
//   listed below, never an app (apps/ui3, apps/sites) or a UI framework; tests may also import vitest
//   and node builtins, and validate/vite.mjs node builtins only.

import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { describe, expect, it } from "vitest";

const ROOT = __dirname;
const CONFIG = new Set(["README.md", "tsconfig.json", "globals.d.ts", "validate/vite.mjs"]);
const PACKAGES = new Set(["zod", "viem", "viem/accounts", "@walletconnect/sign-client", "qrcode-generator", "dcl-validate-impl"]);

const walk = (dir: string): string[] =>
  readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? walk(path) : [relative(ROOT, path)];
  });
const all = walk(ROOT);
const isTest = (file: string) => file.endsWith(".test.ts");
const modules = all.filter((file) => file.endsWith(".ts") && !isTest(file) && !CONFIG.has(file));
const tests = all.filter((file) => isTest(file) && file !== "boundary.test.ts");

// Every way a file can name another, read from the raw source (a commented-out import counts).
function specifiers(file: string): string[] {
  const source = readFileSync(join(ROOT, file), "utf8");
  return [
    ...source.matchAll(/^\s*(?:import|export)\s+(?:[^'";]*?\sfrom\s+)?['"]([^'"]+)['"]/gm),
    ...source.matchAll(/\bimport\(\s*['"]([^'"]+)['"]\s*\)/g),
    ...source.matchAll(/\bvi\.(?:mock|doMock)\(\s*['"]([^'"]+)['"]/g),
  ].map((m) => m[1] as string);
}

function outside(file: string, allowed: (spec: string) => boolean): string[] {
  return specifiers(file).filter((spec) =>
    spec.startsWith(".") ? !resolve(dirname(join(ROOT, file)), spec).startsWith(ROOT + "/") : !allowed(spec),
  );
}

describe("library/web-core", () => {
  it("holds TypeScript modules, their tests and its own config only", () => {
    expect(modules.length).toBeGreaterThan(0);
    expect(all.filter((file) => !file.endsWith(".ts") && !CONFIG.has(file))).toEqual([]);
  });

  it.each(modules)("%s imports only the library and its packages", (file) => {
    expect(outside(file, (spec) => PACKAGES.has(spec))).toEqual([]);
  });

  it.each(tests)("%s imports only the library, its packages, vitest and node", (file) => {
    expect(outside(file, (spec) => PACKAGES.has(spec) || spec === "vitest" || spec.startsWith("node:"))).toEqual([]);
  });

  it("validate/vite.mjs imports node builtins only", () => {
    expect(outside("validate/vite.mjs", (spec) => spec.startsWith("node:"))).toEqual([]);
  });
});
