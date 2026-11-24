// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from "vitest";
import { renderToString } from "react-dom/server";
import { fireEvent, render, cleanup } from "@testing-library/react";
import { LegalPageLayout } from "@dcl/one-ui/apps/sites";
import {
  canonicalLegalTree,
  withoutCurrentMarks,
} from "./canonical.test-utils";
import { LEGAL_DOCS, LEGAL_SIDEBAR, legalPath, resolveLegalKey } from "./index";
import type { LegalDocument } from "./types";

const FIXTURES = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  "fixtures",
);

vi.hoisted(() =>
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true }),
);
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});
const layout = ({ title, toc, intro, sections, cta }: LegalDocument) => ({
  title,
  toc,
  intro,
  sections,
  cta,
});
const sidebar = (active: string) =>
  LEGAL_SIDEBAR.map(({ key, label }) => ({
    href: legalPath(key),
    label,
    icon: "explore" as const,
    active: key === active,
  }));

describe("when rendering the pinned legal documents", () => {
  describe.each(Object.values(LEGAL_DOCS))("and the document is $slug", (doc) => {
    let source: { canonical: unknown };
    let root: HTMLDivElement;

    beforeEach(() => {
      source = JSON.parse(
        readFileSync(
          path.join(FIXTURES, `${doc.slug}.json`),
          "utf8",
        ),
      );
      root = document.createElement("div");
      root.innerHTML = renderToString(
        <LegalPageLayout
          {...layout(doc)}
          sidebar={sidebar(doc.key)}
          sidebarLabel="Legal documents"
        />,
      );
    });

    it("should preserve the captured text, structure and links", () => {
      expect(
        withoutCurrentMarks(canonicalLegalTree(root), legalPath(doc.key)),
      ).toEqual(source.canonical);
    });

    it("should serve it at the path the sidebar links", () => {
      expect(legalPath(doc.key)).toBe(`/${doc.slug}`);
    });

    it("should resolve its slug to its key", () => {
      expect(resolveLegalKey(doc.slug)).toBe(doc.key);
    });

    it("should resolve its key to itself", () => {
      expect(resolveLegalKey(doc.key)).toBe(doc.key);
    });
  });

  describe("and listing the sidebar", () => {
    it("should list Creator Fund between Rewards Program and Credits, as source", () => {
      expect(LEGAL_SIDEBAR.map(({ slug }) => slug)).toEqual([
        "brand",
        "content",
        "ethics",
        "rewards-terms",
        "creator-fund-terms",
        "credits-terms",
        "security",
        "privacy",
        "referral-terms",
        "terms",
      ]);
    });

    it("should label Creator Fund with its slug and icon", () => {
      expect(LEGAL_SIDEBAR.find(({ key }) => key === "creatorFund")).toEqual({
        key: "creatorFund",
        slug: "creator-fund-terms",
        label: "Creator Fund",
        icon: "paid",
      });
    });
  });

  describe("and a block is an ordered list", () => {
    let view: ReturnType<typeof render>;

    beforeEach(() => {
      view = render(
        <LegalPageLayout
          {...layout(LEGAL_DOCS.creatorFund)}
          sidebar={sidebar("creatorFund")}
          sidebarLabel="Legal documents"
        />,
      );
    });

    it("should render it as an ol with its items", () => {
      const list = view.container.querySelector("#eligibility ol");
      expect(list?.querySelectorAll("li")).toHaveLength(5);
    });

    it("should not render it as a ul", () => {
      expect(view.container.querySelectorAll("#eligibility ul")).toHaveLength(0);
    });
  });

  describe("and resolving the content-policy alias", () => {
    it("should resolve it to the content policy", () => {
      expect(resolveLegalKey("content-policy")).toBe("content");
    });
  });

  describe("and the path is unknown or inherited", () => {
    it.each(["missing", "__proto__", "constructor", "toString", ""])(
      "should reject %s",
      (slug) => {
        expect(resolveLegalKey(slug)).toBeNull();
      },
    );
  });

  describe.each(["javascript:alert(1)", "data:text/html,x", "//evil.example"])(
    "and a link points to %s",
    (href) => {
      let view: ReturnType<typeof render>;

      beforeEach(() => {
        const doc = {
          ...LEGAL_DOCS.security,
          intro: [
            {
              type: "p" as const,
              content: [{ type: "link" as const, href, children: ["unsafe"] }],
            },
          ],
          cta: { href, label: "unsafe cta" },
        };
        view = render(
          <LegalPageLayout
            {...layout(doc)}
            sidebar={sidebar("security")}
            sidebarLabel="Legal documents"
          />,
        );
      });

      it("should render the link as its text", () => {
        expect(view.getByText("unsafe").closest("a")).toBeNull();
      });

      it("should not render the call to action", () => {
        expect(view.queryByText("unsafe cta")).toBeNull();
      });
    },
  );

  describe("and the sidebar is navigated", () => {
    let onNavigate: Mock<(href: string) => void>;
    let link: HTMLElement;

    beforeEach(() => {
      onNavigate = vi.fn();
      const view = render(
        <LegalPageLayout
          {...layout(LEGAL_DOCS.privacy)}
          sidebar={sidebar("privacy")}
          sidebarLabel="Legal documents"
          onNavigate={onNavigate}
        />,
      );
      link = view.getByRole("link", { name: "Terms of Use" });
    });

    describe("and the click is an unmodified primary click", () => {
      it("should intercept it", () => {
        fireEvent.click(link, { button: 0 });
        expect(onNavigate).toHaveBeenCalledWith("/terms");
      });
    });

    describe("and the click is modified or not primary", () => {
      it("should not intercept it", () => {
        fireEvent.click(link, { button: 0, ctrlKey: true });
        fireEvent.click(link, { button: 1 });
        expect(onNavigate).not.toHaveBeenCalled();
      });
    });
  });
});
