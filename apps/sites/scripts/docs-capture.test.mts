import { describe, expect, it } from "vitest";

import {
  buildNav, englishMatch, groupTitles, localUrl, mapBounded, pageFile, parseSitemap, parseSitemapIndex, splitCaptured,
  translationsOf, unescapeUrls, variantOf, variantSpace,
} from "./docs-capture.mts";

const LIVE = "https://docs.decentraland.org";
const variant = (p: string) => variantOf(`${LIVE}/${p}/sitemap-pages.xml`)!;

describe("sitemaps", () => {
  it("reads the index and a page sitemap", () => {
    const index = `<?xml version="1.0"?><sitemapindex><sitemap>\n<loc>${LIVE}/sitemap-pages.xml</loc>\n</sitemap><sitemap><loc>${LIVE}/player/player-pt/sitemap-pages.xml</loc></sitemap></sitemapindex>`;
    expect(parseSitemapIndex(index)).toEqual([`${LIVE}/sitemap-pages.xml`, `${LIVE}/player/player-pt/sitemap-pages.xml`]);
    const pages = `<urlset><url>\n<loc>${LIVE}/player/player-pt</loc><priority>1</priority><lastmod>2026-10-13T13:01:02.360Z</lastmod></url><url><loc>${LIVE}/player/player-pt/a?x=1&amp;y=2</loc></url></urlset>`;
    expect(parseSitemap(pages)).toEqual([
      { loc: `${LIVE}/player/player-pt`, lastmod: "2026-10-13T13:01:02.360Z" },
      { loc: `${LIVE}/player/player-pt/a?x=1&y=2`, lastmod: null },
    ]);
  });

  it("names locale variants and leaves English spaces to the mirror", () => {
    expect(variantOf(`${LIVE}/creator/content-creator-pt/sitemap-pages.xml`)).toEqual({
      id: "content-creator-pt", section: "creator", language: "pt-br", label: "PT", path: "creator/content-creator-pt",
      sitemap: `${LIVE}/creator/content-creator-pt/sitemap-pages.xml`,
    });
    expect(variantOf(`${LIVE}/contributor/contributor-zh/sitemap-pages.xml`)).toMatchObject({ id: "contributor-zh", language: "zh", label: "ZH" });
    expect(variantOf(`${LIVE}/sitemap-pages.xml`)).toBeNull();
    expect(variantOf(`${LIVE}/creator/sitemap-pages.xml`)).toBeNull();
    expect(variantOf(`${LIVE}/creator/player-pt/sitemap-pages.xml`)).toBeNull();
  });
});

describe("paths", () => {
  const variants = [variant("player/player-es"), variant("creator/content-creator-ko")];

  it("maps live URLs and site paths of captured variants to /docs", () => {
    expect(localUrl(`${LIVE}/player/player-es`, variants)).toBe("/docs/player/player-es/");
    expect(localUrl("/player/player-es/app-movil/mobile-app.md", variants)).toBe("/docs/player/player-es/app-movil/mobile-app/");
    expect(localUrl(`${LIVE}/creator/content-creator-ko/sdk7/a.md#ids`, variants)).toBe("/docs/creator/content-creator-ko/sdk7/a/#ids");
    expect(localUrl("/creator/scenes-sdk7/a", variants)).toBeNull();
    expect(localUrl("/player/player-esx/a", variants)).toBeNull();
    expect(localUrl("https://github.com/decentraland", variants)).toBeNull();
    expect(localUrl("images/a.png", variants)).toBeNull();
  });

  it("stores a page URL as its folder's index.md", () => {
    expect(pageFile("/docs/player/player-es/")).toBe("player/player-es/index.md");
    expect(pageFile("/docs/player/player-es/a/b/")).toBe("player/player-es/a/b/index.md");
  });

  it("matches English pages on the last segment, shared segments breaking ties", () => {
    const urls = ["/docs/creator/sdk7/examples/", "/docs/creator/editor/examples/", "/docs/creator/glossary/"];
    expect(englishMatch(`${LIVE}/creator/glossary`, urls)).toBe("/docs/creator/glossary/");
    expect(englishMatch(`${LIVE}/creator/sdk7/examples`, urls)).toBe("/docs/creator/sdk7/examples/");
    expect(englishMatch(`${LIVE}/creator/other/examples`, urls)).toBeNull();
    expect(englishMatch(`${LIVE}/creator/missing`, urls)).toBeNull();
  });
});

describe("navigation", () => {
  it("reads the sidebar group titles", () => {
    const html = `<aside data-testid="table-of-contents"><ul><li class="page-document-item [.page-group-item+&amp;]:mt-4"><a href="/x"><span>Home</span></a></li>`
      + `<li class="page-group-item flex"><div><svg><path/></svg>Tutoriais &amp; exemplos</div><ul><li><a href="/y"><span>Y</span></a></li></ul></li>`
      + `<li class="page-group-item"><div>Recompensas</div></li></ul></aside><aside>other</aside>`;
    expect(groupTitles(html)).toEqual(["Tutoriais & exemplos", "Recompensas"]);
    expect(groupTitles("<html></html>")).toEqual([]);
  });

  it("nests pages under groups and parent pages", () => {
    const root = "/docs/creator/content-creator-pt/";
    const pages = [
      { url: root, title: "Bem-vindo" },
      { url: `${root}glossary/`, title: "Glossário" },
      { url: `${root}tutoriais/examples/`, title: "Exemplos" },
      { url: `${root}tutoriais/start/`, title: "Comece" },
      { url: `${root}tutoriais/start/template/`, title: "Modelo" },
      { url: `${root}recompensas/overview/`, title: "Visão geral" },
    ];
    const nav = buildNav(root, pages, ["Tutoriais e exemplos", "Recompensas"]);
    expect(nav.children.map((n) => [n.type, n.title])).toEqual([
      ["item", "Bem-vindo"], ["item", "Glossário"], ["section", "Tutoriais e exemplos"], ["section", "Recompensas"],
    ]);
    expect(nav.children[2].children.map((n) => n.title)).toEqual(["Exemplos", "Comece"]);
    expect(nav.children[2].children[1].children).toEqual([{ type: "item", title: "Modelo", url: `${root}tutoriais/start/template/`, children: [] }]);
    expect(buildNav(root, pages, ["only one"]).children[2].title).toBe("Tutoriais");
  });
});

describe("splitCaptured", () => {
  const md = "> For the complete documentation index, see [llms.txt](https://docs.decentraland.org/llms.txt). More.\n\n# Acerca de\n\nResumen general\n\nBienvenido.\n";

  it("drops the index banner and lifts the description when the English page has one", () => {
    expect(splitCaptured(md, true)).toEqual({ title: "Acerca de", description: "Resumen general", body: "# Acerca de\n\nBienvenido." });
    expect(splitCaptured(md, false)).toEqual({ title: "Acerca de", description: null, body: "# Acerca de\n\nResumen general\n\nBienvenido." });
  });

  it("never takes a block for the description", () => {
    expect(splitCaptured("# T\n\n{% hint style=\"info\" %}\nx\n{% endhint %}\n", true).description).toBeNull();
    expect(splitCaptured("# T\n\n| a | b |\n", true).description).toBeNull();
  });
});

describe("unescapeUrls", () => {
  it("unescapes ampersands in URLs only", () => {
    expect(unescapeUrls("![a](https://x.io/o/a.png?alt=media\\&token=1) and `a \\& b`"))
      .toBe("![a](https://x.io/o/a.png?alt=media&token=1) and `a \\& b`");
  });
});

describe("manifest", () => {
  const es = variant("player/player-es");
  const pt = variant("player/player-pt");

  it("records a captured space with its source, date and sitemap", () => {
    expect(variantSpace(pt, { id: "player", title: "Players", blurb: "Play.", icon: "home" as const }, "2026-11-09T12:00:00.000Z")).toEqual({
      id: "player-pt", title: "Players", blurb: "Play.", icon: "home", path: "player/player-pt", section: "player", language: "pt-br",
      label: "PT", source: "capture", capturedAt: "2026-11-09T12:00:00.000Z", sitemap: `${LIVE}/player/player-pt/sitemap-pages.xml`,
    });
  });

  it("pairs translations by position and links English both ways", () => {
    const t = translationsOf([
      { variant: es, pages: [{ url: "/docs/player/player-es/", en: "/docs/player/" }, { url: "/docs/player/player-es/x/", en: null }] },
      { variant: pt, pages: [{ url: "/docs/player/player-pt/", en: "/docs/player/" }, { url: "/docs/player/player-pt/y/", en: null }] },
    ]);
    expect(t.get("/docs/player/player-es/")).toEqual({ PT: "/docs/player/player-pt/", EN: "/docs/player/" });
    expect(t.get("/docs/player/player-pt/y/")).toEqual({ ES: "/docs/player/player-es/x/" });
    expect(t.get("/docs/player/")).toEqual({ ES: "/docs/player/player-es/", PT: "/docs/player/player-pt/" });
  });
});

describe("mapBounded", () => {
  it("keeps order and never runs more than the limit", async () => {
    let running = 0, peak = 0;
    const out = await mapBounded([5, 1, 3, 2], 2, async (n) => {
      running++; peak = Math.max(peak, running);
      await new Promise((r) => setTimeout(r, n));
      running--;
      return n * 10;
    });
    expect(out).toEqual([50, 10, 30, 20]);
    expect(peak).toBe(2);
  });
});
