// @vitest-environment jsdom
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { createRoutesStub } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true });
});

const track = vi.hoisted(() => vi.fn());
vi.mock("@/core/lib/telemetry/track", () => ({ track }));
vi.mock("@ui/web/frames/SitesChrome", () => ({
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

afterEach(() => {
  cleanup();
  track.mockClear();
});

describe("when a legal page is read", () => {
  it("should track one view per document, also after sidebar navigation", async () => {
    render(<Stub initialEntries={["/privacy"]} />);
    await screen.findByRole("heading", { level: 1, name: LEGAL_DOCS.privacy.title });

    await userEvent.click(screen.getByRole("link", { name: "Terms of Use" }));
    await screen.findByRole("heading", { level: 1, name: LEGAL_DOCS.terms.title });

    await waitFor(() =>
      expect(calls("lp_legal_viewed")).toEqual([{ doc: "privacy" }, { doc: "terms" }]),
    );
  });

  it("should track a contents click with its section", async () => {
    const [entry] = LEGAL_DOCS.privacy.toc;
    render(<Stub initialEntries={["/privacy"]} />);
    await screen.findByRole("heading", { level: 1, name: LEGAL_DOCS.privacy.title });

    await userEvent.click(screen.getAllByRole("link", { name: entry.label })[0]);

    expect(calls("lp_legal_section_clicked")).toEqual([
      { doc: "privacy", section: entry.id },
    ]);
  });
});
