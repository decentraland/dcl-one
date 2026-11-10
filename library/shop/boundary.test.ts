// library/shop is the Shop's portable core: the web Shop, Sites and the Explorer HUD import it, each from
// its own host. What this file holds it to:
//
// - it is TypeScript modules only (no components, scripts or styles);
// - its modules import only the core itself, the catalyst server's DTOs (@catalyst-api) and viem; its
//   tests may also import vitest and node builtins;
// - it reaches for no host: it compiles under library/shop/tsconfig.json, ES only plus the globals in
//   globals.d.ts, so window, document, fetch, storage, location, WebSocket, process or import.meta.env
//   do not compile (apps/ui3's typecheck runs that profile). Tests run in node and are not held to it;
// - every module loads in node, with no DOM.

import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { describe, expect, it } from "vitest";

const ROOT = __dirname;

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? walk(path) : [relative(ROOT, path)];
  });
}

const all = walk(ROOT);
const CONFIG = new Set(["README.md", "tsconfig.json", "globals.d.ts"]);
const isTest = (file: string): boolean => file.endsWith(".test.ts");
const modules = all.filter((file) => file.endsWith(".ts") && !isTest(file) && !CONFIG.has(file));
// This file is the checker: its patterns spell out the very forms it looks for.
const tests = all.filter((file) => isTest(file) && file !== "boundary.test.ts");

// Every way a module can name another: static imports and re-exports (side-effect imports included),
// dynamic imports and require. Read from the raw source, so a commented-out import counts as one: that
// can only fail the check, never let something through.
function specifiers(source: string): { named: string[]; opaque: string[] } {
  const named = [
    ...source.matchAll(/^\s*(?:import|export)\s+(?:[^'";]*?\sfrom\s+)?['"]([^'"]+)['"]/gm),
    ...source.matchAll(/\bimport\(\s*['"]([^'"]+)['"]\s*\)/g),
  ].map((m) => m[1] as string);
  const opaque = [
    ...[...source.matchAll(/\bimport\(\s*([^'"\s][^)]*)\)/g)].map((m) => `import(${m[1]})`),
    ...[...source.matchAll(/\brequire\(([^)]*)\)/g)].map((m) => `require(${m[1]})`),
  ];
  return { named, opaque };
}

function outside(file: string, allowed: (spec: string) => boolean): string[] {
  const { named, opaque } = specifiers(readFileSync(join(ROOT, file), "utf8"));
  return [
    ...named.filter((spec) =>
      spec.startsWith(".") ? !resolve(dirname(join(ROOT, file)), spec).startsWith(ROOT + "/") : !allowed(spec),
    ),
    ...opaque,
  ];
}

const corePackage = (spec: string): boolean => spec === "viem" || spec.startsWith("@catalyst-api/");

// tsconfig.json carries // comments on their own lines only.
const profile = JSON.parse(
  readFileSync(join(ROOT, "tsconfig.json"), "utf8")
    .split("\n")
    .filter((line) => !line.trim().startsWith("//"))
    .join("\n"),
) as { compilerOptions: { lib: string[]; types: string[] }; include: string[]; exclude: string[] };

describe("library/shop", () => {
  it("holds TypeScript modules, their tests and its own config only", () => {
    expect(modules.length).toBeGreaterThan(0);
    expect(all.filter((file) => !file.endsWith(".ts") && !CONFIG.has(file))).toEqual([]);
  });

  it.each(modules)("%s imports only the core, @catalyst-api and viem", (file) => {
    expect(outside(file, corePackage)).toEqual([]);
  });

  it.each(tests)("%s imports only the core, @catalyst-api, viem, vitest and node", (file) => {
    expect(outside(file, (spec) => corePackage(spec) || spec === "vitest" || spec.startsWith("node:"))).toEqual([]);
  });

  it("compiles its modules against ES and the shared globals alone", () => {
    expect(profile.compilerOptions.lib).toEqual(["ES2023"]);
    expect(profile.compilerOptions.types).toEqual([]);
    expect(profile.include).toEqual(["."]);
    expect(profile.exclude).toEqual(["**/*.test.ts"]);
    const declared = [...readFileSync(join(ROOT, "globals.d.ts"), "utf8").matchAll(/^declare\s+\w+\s+(\w+)/gm)];
    expect(declared.map((m) => m[1])).toEqual(["console", "TextEncoder", "URL", "URLSearchParams", "AbortSignal"]);
  });

  // This project runs in node, with no DOM: a module that touched one while loading would throw here.
  const loaders = import.meta.glob(["./**/*.ts", "!./**/*.test.ts", "!./**/*.d.ts"]);
  it("loads every module", () => {
    expect(
      Object.keys(loaders)
        .map((key) => key.slice(2))
        .sort(),
    ).toEqual([...modules].sort());
  });
  it.each(Object.keys(loaders))("%s loads without a DOM", async (key) => {
    expect(typeof globalThis.window).toBe("undefined");
    await expect(loaders[key]!()).resolves.toBeTypeOf("object");
  });
});
