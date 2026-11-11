import { renderToString } from "react-dom/server";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";
import { ChromeNavContext } from "@ui/web/frames/chrome-nav";
import type { DocsPageView } from "@/data/lib/docs/types";
import DocsPage from "./DocsPage";
const space = {
  id: "player",
  title: "Player",
  blurb: "",
  icon: "home" as const,
};
const view: DocsPageView = {
  url: "/docs/",
  title: "Docs",
  description: null,
  mdUrl: "/docs/index.md",
  editUrl: null,
  updatedAt: null,
  space,
  spaces: [space],
  nav: { children: [] },
  navUrl: "/docs/",
  breadcrumb: [],
  prev: null,
  next: null,
  html: "<p>Documentation</p>",
  toc: [],
  languages: [],
};
describe("when Docs adapts its offsets to the owning header layout", () => {
  it.each(["native", "legacy"] as const)(
    "should expose the %s policy to both sticky layout roots",
    (layout) => {
      const html = renderToString(
        <MemoryRouter>
          <ChromeNavContext.Provider
            value={{ headerLayout: layout === "native" ? "native" : undefined }}
          >
            <DocsPage view={view} />
          </ChromeNavContext.Provider>
        </MemoryRouter>,
      );
      expect(html).toMatch(
        new RegExp(
          `class="[^"]*docs-topbar[^"]*" data-header-layout="${layout}"`,
        ),
      );
      expect(html).toMatch(
        new RegExp(`class="[^"]*dcl-docs[^"]*" data-header-layout="${layout}"`),
      );
    },
  );
});
