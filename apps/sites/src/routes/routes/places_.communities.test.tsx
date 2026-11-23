// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/react";
import { createMemoryRouter, Outlet, RouterProvider } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import { page } from "@/core/lib/telemetry/track";

import PlacesCommunities from "./places_.communities";

vi.hoisted(() => Object.assign(window, { __vite_plugin_react_preamble_installed__: true }));

vi.mock("@/core/lib/telemetry/track", () => ({ page: vi.fn(), adoptSitesSegmentConfig: vi.fn() }));
vi.mock("./social", () => ({ default: () => <p>communities list</p>, meta: () => [] }));

function renderAt(root: Record<string, unknown>) {
  const router = createMemoryRouter(
    [{ id: "root", path: "/", Component: Outlet, children: [{ path: "places/communities", Component: PlacesCommunities }] }],
    { initialEntries: ["/places/communities"], hydrationData: { loaderData: { root } } },
  );
  return render(<RouterProvider router={router} />);
}

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("/places/communities (decentraland/sites DiscoverCommunitiesPage.tsx:28)", () => {
  it("should show the communities list and send the legacy page view once, under its exact name", () => {
    const view = renderAt({ sid: "sid-root", analyticsUserId: "0xabc" });
    expect(view.getByText("communities list")).toBeTruthy();
    expect(vi.mocked(page).mock.calls).toEqual([["Communities — Decentraland", {}, { sid: "sid-root", userId: "0xabc" }]]);
  });

  it("should send no page view without a session", () => {
    renderAt({ sid: "", analyticsUserId: "" });
    expect(vi.mocked(page)).not.toHaveBeenCalled();
  });
});
