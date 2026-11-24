import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { expect, it } from "vitest";

// Every `docsUrl("…")` the web apps link (the footer of each Sites page, the invite and /create FAQs, the Creator
// Hub) must name a page of the committed docs tree: /docs answers 404 for any other address.
const SOURCES = ["src", "../ui3/src", "../../library/ui/apps/sites"];

function docsLinks(): Array<{ file: string; url: string }> {
  const links: Array<{ file: string; url: string }> = [];
  for (const root of SOURCES) {
    for (const name of readdirSync(root, { recursive: true, encoding: "utf8" })) {
      if (!/\.tsx?$/.test(name) || /\.test\.tsx?$/.test(name)) continue;
      const file = path.join(root, name);
      for (const [, , link] of readFileSync(file, "utf8").matchAll(/docsUrl\((["'])([^"']*)\1\)/g)) {
        const page = link.split("#")[0].replace(/^\/+|\/+$/g, "");
        links.push({ file, url: page ? `/docs/${page}/` : "/docs/" });
      }
    }
  }
  return links;
}

it("every docs link in the web apps names a page of the docs tree", () => {
  const manifest = JSON.parse(readFileSync("content/docs/manifest.json", "utf8")) as { pages: Array<{ url: string }> };
  const pages = new Set(manifest.pages.map((page) => page.url));
  const links = docsLinks();
  expect(links.map((link) => link.url)).toEqual(expect.arrayContaining(["/docs/player/introduction/about-decentraland/", "/docs/player/faqs/"]));
  expect(links.filter((link) => !pages.has(link.url))).toEqual([]);
});
