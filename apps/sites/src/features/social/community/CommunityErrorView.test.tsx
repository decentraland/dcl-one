// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { createMemoryRouter, data, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});
const net = vi.hoisted(() => ({ fetchCommunity: vi.fn() }));
vi.mock("@/data/lib/catalyst/social/community-detail", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/social/community-detail")>()),
  fetchCommunity: net.fetchCommunity,
}));
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => ({ identity: null }) }));
vi.mock("@ui/web/frames/SitesChrome", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));
vi.mock("../../events/useCreatorProfiles", () => ({ useCreatorProfiles: () => ({}) }));
vi.mock("../../events/useEventReminders", () => ({
  useEventReminders: () => ({ get: () => ({ active: false, pending: false, toggle: () => {} }) }),
}));

import { CatalystError } from "@/data/lib/catalyst/client";
import { CommunityNotFoundError } from "@/data/lib/catalyst/social/community-detail";
import { communityKeys } from "@/data/lib/community/queries";

import { CommunityErrorView } from "./CommunityErrorView";
import { CommunityPage } from "./CommunityPage";

const ID = "e99471aa-31c4-4952-abf6-99905445f43b";
let client: QueryClient;

function show(loader?: () => unknown, element: ReactNode = <CommunityPage id={ID} now={0} />) {
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter(
    [
      { path: "/social/communities/:id", loader, element, ErrorBoundary: CommunityErrorView },
      { path: "/elsewhere", element: <p>Elsewhere</p> },
    ],
    { initialEntries: [`/social/communities/${ID}`] },
  );
  render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return router;
}

beforeEach(() => net.fetchCommunity.mockReset());
afterEach(() => cleanup());

const community = {
  id: ID,
  name: "Decentraland Foundation",
  description: null,
  ownerAddress: "0xbcac4dafb7e215f2f6cb3312af6d5e4f9d9e7eda",
  ownerName: null,
  privacy: "private" as const,
  membersCount: 1,
  thumbnailUrl: null,
  role: null,
};

describe("CommunityErrorView", () => {
  it("renders a document request's 404 as not found, with no retry", async () => {
    show(() => {
      throw data({ reason: "not-found" }, { status: 404 });
    });

    expect(await screen.findByRole("heading", { name: "Community not found" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Try again" })).toBeNull();
  });

  it("renders a document request's 503 as unavailable, with a retry", async () => {
    show(() => {
      throw data({ reason: "unavailable" }, { status: 503 });
    });

    expect(await screen.findByRole("heading", { name: "Community unavailable" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
  });

  it("renders a client navigation's unknown community as not found", async () => {
    net.fetchCommunity.mockRejectedValueOnce(new CommunityNotFoundError(ID));
    show();

    expect(await screen.findByRole("heading", { name: "Community not found" })).toBeTruthy();
  });

  it("retries a client navigation's failed read and renders the community", async () => {
    net.fetchCommunity.mockRejectedValueOnce(new CatalystError("502", "/v1/communities", 502)).mockResolvedValue(community);
    show();

    await userEvent.click(await screen.findByRole("button", { name: "Try again" }));

    expect(await screen.findByRole("heading", { level: 1, name: "Decentraland Foundation" })).toBeTruthy();
  });

  it("releases the failed read when the visitor leaves", async () => {
    net.fetchCommunity.mockRejectedValueOnce(new CatalystError("502", "/v1/communities", 502));
    const router = show();
    await screen.findByRole("heading", { name: "Community unavailable" });

    await router.navigate("/elsewhere");

    await waitFor(() => expect(client.getQueryState(communityKeys.detail(ID))?.status).toBe("pending"));
  });

  it("passes any other error up to the root boundary", async () => {
    const Root = () => {
      throw new Error("bug");
    };
    client = new QueryClient();
    const router = createMemoryRouter(
      [
        {
          path: "/",
          ErrorBoundary: () => <p>Root boundary</p>,
          children: [{ path: "social/communities/:id", element: <Root />, ErrorBoundary: CommunityErrorView }],
        },
      ],
      { initialEntries: [`/social/communities/${ID}`] },
    );
    vi.spyOn(console, "error").mockImplementation(() => {});
    render(
      <QueryClientProvider client={client}>
        <RouterProvider router={router} />
      </QueryClientProvider>,
    );

    expect(await screen.findByText("Root boundary")).toBeTruthy();
  });
});
