import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import config from "../vite.config";
import { LEGAL_DOCS } from "../src/data/legal";
const matched = (pathname: string) =>
  Object.keys(config.server?.proxy ?? {}).filter((prefix) =>
    prefix.startsWith("^")
      ? new RegExp(prefix).test(pathname)
      : pathname.startsWith(prefix),
  );
describe("when legal pages share a prefix with the node credits API", () => {
  it.each(Object.values(LEGAL_DOCS))(
    "should keep /$slug on the Sites router",
    (doc) => {
      expect(matched(`/${doc.slug}`)).toEqual([]);
      expect(matched(`/${doc.slug}?language=en`)).toEqual([]);
    },
  );
  it.each(["/credits", "/credits?limit=1", "/credits/account"])(
    "should preserve the node API proxy at %s",
    (pathname) => {
      expect(matched(pathname)).toContain("^/credits(?:[/?]|$)");
    },
  );
});

describe("when Sites resolves the public foundations stylesheet", () => {
  it("should map every public stylesheet export to its source file in Vite and TypeScript", () => {
    const uiPackage = JSON.parse(
      readFileSync(
        resolve(import.meta.dirname, "../../../library/ui/package.json"),
        "utf8",
      ),
    ) as { exports: Record<string, string> };
    const siteTsconfig = readFileSync(
      resolve(import.meta.dirname, "../tsconfig.json"),
      "utf8",
    );
    const viteAliases = config.resolve?.alias as Record<string, string>;
    const uiAlias = viteAliases["@dcl/one-ui"];
    const uiRoot = resolve(import.meta.dirname, "../../../library/ui");
    const sitesRoot = resolve(import.meta.dirname, "..");

    expect(uiAlias).toBe(uiRoot);
    expect(resolve(uiAlias, uiPackage.exports["."].slice(2))).toBe(
      resolve(uiRoot, uiPackage.exports["."]),
    );
    expect(resolve(uiAlias, "foundations/index.ts")).toBe(
      resolve(uiRoot, uiPackage.exports["./foundations"]),
    );

    for (const [subpath, target] of Object.entries(uiPackage.exports).filter(
      ([, target]) => target.endsWith(".css"),
    )) {
      const specifier = `@dcl/one-ui/${subpath.slice(2)}`;
      const expectedSource = resolve(uiRoot, target);
      const escapedSpecifier = specifier.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
      const typePath = siteTsconfig.match(
        new RegExp(`"${escapedSpecifier}"\\s*:\\s*\\[\\s*"([^"]+)"\\s*\\]`),
      )?.[1];

      expect(viteAliases[specifier]).toBe(expectedSource);
      expect(typePath).toBeDefined();
      expect(resolve(sitesRoot, typePath ?? "")).toBe(expectedSource);
    }
  });
});
