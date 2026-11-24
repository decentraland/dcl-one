// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { createMemoryRouter, Outlet, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { page } from "@/core/lib/telemetry/track";
import { fetchCommunityDirectory, type DirectoryCommunity } from "@/data/lib/catalyst/social/communities";

import PlacesCommunities from "./places_.communities";

vi.hoisted(() => Object.assign(window, { __vite_plugin_react_preamble_installed__: true }));

vi.mock("@/core/lib/telemetry/track", () => ({ page: vi.fn(), adoptSitesSegmentConfig: vi.fn() }));
vi.mock("@/features/components/chrome/SitesLayout", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));
vi.mock("@/data/lib/catalyst/social/communities", () => ({ fetchCommunityDirectory: vi.fn() }));

const community = (id: string, name: string, membersCount: number): DirectoryCommunity => ({
  id,
  name,
  description: null,
  ownerName: "Owner",
  thumbnailUrl: null,
  membersCount,
  privacy: "public",
});

function renderAt(root: Record<string, unknown> = { sid: "sid-root", analyticsUserId: "0xabc" }) {
  const router = createMemoryRouter(
    [{ id: "root", path: "/", Component: Outlet, children: [{ path: "places/communities", Component: PlacesCommunities }] }],
    { initialEntries: ["/places/communities"], hydrationData: { loaderData: { root } } },
  );
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.mocked(fetchCommunityDirectory).mockResolvedValue([community("c-2", "Big Club", 1340), community("c-1", "Solo", 1)]);
});

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("/places/communities (decentraland/sites pages/places/DiscoverCommunitiesPage.tsx)", () => {
  it("should send the legacy page view once, under its exact name", async () => {
    renderAt();
    await screen.findByRole("link", { name: "Big Club" });
    expect(vi.mocked(page).mock.calls).toEqual([["Communities — Decentraland", {}, { sid: "sid-root", userId: "0xabc" }]]);
  });

  it("should send no page view without a session", async () => {
    renderAt({ sid: "", analyticsUserId: "" });
    await screen.findByRole("link", { name: "Big Club" });
    expect(vi.mocked(page)).not.toHaveBeenCalled();
  });

  it("should list the communities in the order read, each linking to its community page with its member count", async () => {
    renderAt();
    expect(screen.getByRole("heading", { name: "Communities" })).toBeTruthy();
    const links = await screen.findAllByRole("link", { name: /Big Club|Solo/ });
    expect(links.map((link) => [link.textContent, link.getAttribute("href")])).toEqual([
      ["Big Club", "/social/communities/c-2"],
      ["Solo", "/social/communities/c-1"],
    ]);
    expect(screen.getByText("1,340 members")).toBeTruthy();
    expect(screen.getByText("1 member")).toBeTruthy();
  });

  it("should read the communities matching what is typed in the search", async () => {
    renderAt();
    await screen.findByRole("link", { name: "Big Club" });
    await userEvent.type(screen.getByRole("textbox", { name: "Search communities" }), " club ");
    await waitFor(() => expect(vi.mocked(fetchCommunityDirectory)).toHaveBeenLastCalledWith("club", expect.anything()));
  });

  it("should say so when no community matches", async () => {
    vi.mocked(fetchCommunityDirectory).mockResolvedValue([]);
    renderAt();
    expect(await screen.findByText("No communities found.")).toBeTruthy();
  });

  it("should offer a retry when the list cannot be read, and show it once read", async () => {
    vi.mocked(fetchCommunityDirectory).mockRejectedValueOnce(new Error("down"));
    renderAt();
    expect((await screen.findByRole("alert")).textContent).toBe("Something went wrong loading communities.");
    await userEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByRole("link", { name: "Big Club" })).toBeTruthy();
  });
});
