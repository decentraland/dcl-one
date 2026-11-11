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
  it("should map the declared package export to its source file in Vite and TypeScript", () => {
    const uiPackage = JSON.parse(
      readFileSync(
        resolve(import.meta.dirname, "../../../library/ui/package.json"),
        "utf8",
      ),
    );
    const siteTsconfig = readFileSync(
      resolve(import.meta.dirname, "../tsconfig.json"),
      "utf8",
    );
    const publicExport = uiPackage.exports["./foundations.css"];
    const sourceFile = resolve(
      import.meta.dirname,
      "../../../library/ui/foundations/foundations.css",
    );
    const viteAlias = (config.resolve?.alias as Record<string, string>)[
      "@dcl/one-ui/foundations.css"
    ];
    const typePath = siteTsconfig.match(
      /"@dcl\/one-ui\/foundations\.css"\s*:\s*\[\s*"([^"]+)"\s*\]/,
    )?.[1];

    expect(publicExport).toBe("./foundations/foundations.css");
    expect(viteAlias).toBe(sourceFile);
    expect(typePath).toBe("../../library/ui/foundations/foundations.css");
    expect(resolve(import.meta.dirname, "../", typePath ?? "")).toBe(
      sourceFile,
    );
  });
});
