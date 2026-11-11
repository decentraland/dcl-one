// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import path from "node:path";
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

vi.hoisted(() =>
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true }),
);
afterEach(cleanup);
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
          path.resolve("src/data/legal/fixtures", `${doc.slug}.json`),
          "utf8",
        ),
      );
      const root = document.createElement("div");
      root.innerHTML = renderToString(
        <LegalPageLayout
          {...doc}
          sidebar={sidebar(doc.key)}
          sidebarLabel="Legal documents"
        />,
      );
      expect(
        withoutCurrentMarks(canonicalLegalTree(root), legalPath(doc.key)),
      ).toEqual(source.canonical);
    },
  );
  it.each([
    "credits-terms",
    "content-policy",
    "rewards-terms",
    "referral-terms",
  ])("should resolve the existing or canonical document path %s", (slug) => {
    expect(resolveLegalKey(slug)).not.toBeNull();
  });
  it.each(["missing", "__proto__", "constructor", "toString", ""])(
    "should reject unknown or inherited path %s",
    (slug) => {
      expect(resolveLegalKey(slug)).toBeNull();
    },
  );
  it("should intercept only unmodified primary sidebar navigation", () => {
    const onNavigate = vi.fn();
    const view = render(
      <LegalPageLayout
        {...LEGAL_DOCS.privacy}
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
