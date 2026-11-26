// @vitest-environment jsdom
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { createRoutesStub } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true });
});

const track = vi.hoisted(() => vi.fn());
vi.mock("@/core/lib/telemetry/track", () => ({ track }));
vi.mock("@/features/components/chrome/SitesLayout", () => ({
  default: ({ children }: { children?: ReactNode }) => <>{children}</>,
}));

import { LEGAL_DOCS } from "@/data/legal";
import { resolveLegalKey } from "@/data/legal/nav";

import LegalSlugRoute from "./$slug";

const Stub = createRoutesStub([
  {
    path: "/:slug",
    Component: LegalSlugRoute as never,
    loader: ({ params }) => {
      const key = resolveLegalKey(params.slug ?? "");
      if (!key) throw new Response(null, { status: 404 });
      return { doc: key, legal: LEGAL_DOCS[key], sid: "s" };
    },
    HydrateFallback: () => null,
  },
]);

const calls = (event: string) =>
  track.mock.calls
    .filter(([name]) => name === event)
    .map(([, props]) => props);

describe("LegalSlugRoute telemetry", () => {
  afterEach(() => {
    cleanup();
    vi.resetAllMocks();
  });

  describe("when a legal page is read", () => {
    beforeEach(async () => {
      render(<Stub initialEntries={["/privacy"]} />);
      await screen.findByRole("heading", { level: 1, name: LEGAL_DOCS.privacy.title });
    });

    describe("and the reader navigates to another document from the sidebar", () => {
      beforeEach(async () => {
        await userEvent.click(screen.getByRole("link", { name: "Terms of Use" }));
        await screen.findByRole("heading", { level: 1, name: LEGAL_DOCS.terms.title });
      });

      it("should track one view per document", async () => {
        await waitFor(() =>
          expect(calls("lp_legal_viewed")).toEqual([{ doc: "privacy" }, { doc: "terms" }]),
        );
      });
    });

    describe("and the reader clicks a contents entry", () => {
      let entry: (typeof LEGAL_DOCS.privacy.toc)[number];

      beforeEach(async () => {
        [entry] = LEGAL_DOCS.privacy.toc;
        await userEvent.click(screen.getAllByRole("link", { name: entry.label })[0]);
      });

      it("should track the click with its section", () => {
        expect(calls("lp_legal_section_clicked")).toEqual([
          { doc: "privacy", section: entry.id },
        ]);
      });
    });
  });
});
