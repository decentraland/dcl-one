// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import CreatorsShell, { type CreatorsPage } from "./CreatorsShell";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

vi.mock("./hooks/useFeatureFlag", () => ({ useFeatureFlag: () => ({ enabled: false, isLoading: false }) }));
vi.mock("./hooks/useFailureReports", () => ({ useFailureReports: () => {} }));
vi.mock("./components/SubNav", () => ({ SubNav: () => <nav aria-label="Creators sections" /> }));
vi.mock("./components/Toasts", () => ({ Toasts: () => null }));
vi.mock("./components/CollectionsPage", () => ({ CollectionsPage: () => <h1>Collections</h1> }));
vi.mock("./components/ItemEditorPage", () => ({ ItemEditorPage: () => <h1>Item editor</h1> }));
vi.mock("@/data/lib/shop/marketplace-access", () => ({ useMarketplaceHere: () => false }));

function renderShell(page: CreatorsPage) {
  render(
    <QueryClientProvider client={new QueryClient()}>
      <MemoryRouter initialEntries={["/create/collections"]}>
        <CreatorsShell page={page} sid="s1" />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

afterEach(() => cleanup());

describe("when a creators page with the site chrome opens", () => {
  beforeEach(async () => {
    renderShell("collections");
    await screen.findByRole("heading", { name: "Collections" });
  });

  it("should show the Sites top bar", () => {
    const banner = within(screen.getByRole("banner", { name: "Decentraland" }));
    expect(banner.getByRole("button", { name: "Open menu" })).toBeTruthy();
    expect(banner.queryByRole("button", { name: "Toggle navigation menu" })).toBeNull();
  });

  it("should select the page's Create destination in the top bar", () => {
    const current = screen.getByRole("banner").querySelectorAll('a[aria-current="page"]');
    expect([...current].map((link) => link.getAttribute("href"))).toEqual(["/create/collections"]);
  });

  it("should keep the section's field behind the chrome", () => {
    expect(document.querySelector("[data-site-layout] [data-creators]")).toBeTruthy();
  });

  it("should show the site footer", () => {
    expect(screen.getByRole("contentinfo")).toBeTruthy();
  });
});

describe("when the full-screen item editor opens", () => {
  beforeEach(async () => {
    renderShell("editor");
    await screen.findByRole("heading", { name: "Item editor" });
  });

  it("should render without the site chrome", () => {
    expect(screen.queryByRole("banner")).toBeNull();
    expect(screen.queryByRole("contentinfo")).toBeNull();
    expect(document.querySelector("[data-site-layout]")).toBeNull();
  });
});
