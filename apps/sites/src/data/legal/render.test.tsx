// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it, vi } from "vitest";
import { renderToString } from "react-dom/server";
import { fireEvent, render, cleanup } from "@testing-library/react";
import { afterEach } from "vitest";
import { LegalPageLayout } from "@dcl/one-ui/apps/sites/legal/LegalPageLayout";
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
afterEach(cleanup);
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

describe("when rendering the nine pinned legal documents", () => {
  it.each(Object.values(LEGAL_DOCS))(
    "should preserve the captured text, structure and links for $slug",
    (doc) => {
      const source = JSON.parse(
        readFileSync(
          path.join(FIXTURES, `${doc.slug}.json`),
          "utf8",
        ),
      );
      const root = document.createElement("div");
      root.innerHTML = renderToString(
        <LegalPageLayout
          {...layout(doc)}
          sidebar={sidebar(doc.key)}
          sidebarLabel="Legal documents"
        />,
      );
      expect(
        withoutCurrentMarks(canonicalLegalTree(root), legalPath(doc.key)),
      ).toEqual(source.canonical);
    },
  );
  it.each(Object.values(LEGAL_DOCS))(
    "should serve $slug at the path the sidebar links",
    (doc) => {
      expect(legalPath(doc.key)).toBe(`/${doc.slug}`);
      expect(resolveLegalKey(doc.slug)).toBe(doc.key);
      expect(resolveLegalKey(doc.key)).toBe(doc.key);
    },
  );
  it("should resolve the content-policy alias", () => {
    expect(resolveLegalKey("content-policy")).toBe("content");
  });
  it.each(["missing", "__proto__", "constructor", "toString", ""])(
    "should reject unknown or inherited path %s",
    (slug) => {
      expect(resolveLegalKey(slug)).toBeNull();
    },
  );
  it.each(["javascript:alert(1)", "data:text/html,x", "//evil.example"])(
    "should render a link to %s as its text",
    (href) => {
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
      const view = render(
        <LegalPageLayout
          {...layout(doc)}
          sidebar={sidebar("security")}
          sidebarLabel="Legal documents"
        />,
      );
      expect(view.getByText("unsafe").closest("a")).toBeNull();
      expect(view.queryByText("unsafe cta")).toBeNull();
    },
  );
  it("should intercept only unmodified primary sidebar navigation", () => {
    const onNavigate = vi.fn();
    const view = render(
      <LegalPageLayout
        {...layout(LEGAL_DOCS.privacy)}
        sidebar={sidebar("privacy")}
        sidebarLabel="Legal documents"
        onNavigate={onNavigate}
      />,
    );
    const link = view.getByRole("link", { name: "Terms of Use" });
    fireEvent.click(link, { button: 0 });
    expect(onNavigate).toHaveBeenCalledWith("/terms");
    onNavigate.mockClear();
    fireEvent.click(link, { button: 0, ctrlKey: true });
    fireEvent.click(link, { button: 1 });
    expect(onNavigate).not.toHaveBeenCalled();
  });
});
