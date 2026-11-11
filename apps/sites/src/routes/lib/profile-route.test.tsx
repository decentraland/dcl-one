// @vitest-environment jsdom
import { dehydrate, focusManager, QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { createMemoryRouter, RouterProvider, useNavigate } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});
const net = vi.hoisted(() => ({
  fetchProfile: vi.fn(),
  fetchMyCommunities: vi.fn(),
  fetchProfilePlaces: vi.fn(),
  fetchProfileCreations: vi.fn(),
  fetchProfileAssets: vi.fn(),
  track: vi.fn(),
}));
const auth = vi.hoisted(() => ({
  state: { isConnected: false, address: null as string | null, identity: null as unknown },
}));
vi.mock("@/data/lib/catalyst/social/profile", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/social/profile")>()),
  fetchProfile: net.fetchProfile,
}));
vi.mock("@/data/lib/catalyst/social/profile-communities", () => ({ fetchMyCommunities: net.fetchMyCommunities }));
vi.mock("@/data/lib/catalyst/places/profile-places", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/places/profile-places")>()),
  fetchProfilePlaces: net.fetchProfilePlaces,
}));
vi.mock("@/data/lib/catalyst/marketplace/profile-collectibles", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/marketplace/profile-collectibles")>()),
  fetchProfileCreations: net.fetchProfileCreations,
  fetchProfileAssets: net.fetchProfileAssets,
}));
vi.mock("@/data/lib/catalyst/social/badges", () => ({ fetchUserBadges: () => Promise.resolve([]) }));
vi.mock("@/core/lib/telemetry/track", () => ({ track: net.track }));
vi.mock("@/data/lib/auth/index", () => ({ useAuth: () => auth.state }));

// The tab components are today's ui3 pages; these stand-ins show what the route hands them.
const { stub } = vi.hoisted(() => ({
  stub: (tab: string) => ({
    default: (props: { profile: { name: string }; isOwnProfile?: boolean }) => (
      <section data-tab={tab}>
        <h2>{props.profile.name}</h2>
        <p>{props.isOwnProfile ? "own" : "member"}</p>
      </section>
    ),
  }),
}));
vi.mock("@ui/web/pages/StProfileOverviewTab", () => stub("overview"));
vi.mock("@ui/web/pages/StProfilePhotosTab", () => stub("photos"));
vi.mock("@ui/web/pages/StProfileCommunitiesTab", () => stub("communities"));

import { CatalystError } from "@/data/lib/catalyst/client";
import { mapProfile } from "@/data/lib/catalyst/social/profile";
import { profileKeys } from "@/data/lib/profile/queries";

import type { ProfileLoaderData } from "./profile-loader.server";
import { ProfileErrorBoundary, ProfileRoute, profileClientLoader } from "./profile-route";

const A = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const avatar = (name: string) => ({ name, hasClaimedName: true, description: "", avatar: { snapshots: { face256: "" } } });
const identity = { signer: A, ephemeral: { address: "0xeph", privateKey: "0xdeadbeef" }, expiration: "2999-01-01T00:00:00.000Z", authChain: [] };

let client: QueryClient;

function Other() {
  const navigate = useNavigate();
  return <button onClick={() => navigate(`/profile/${A}`)}>back to profile</button>;
}

/** The profile routes as Sites declares them, with the loader the browser runs on a navigation. */
function mount(path: string, loader: () => unknown = profileClientLoader) {
  const route = {
    loader,
    Component: () => <ProfileRoute loaderData={null} />,
    ErrorBoundary: ProfileErrorBoundary,
  };
  // Under a root route with its own boundary, as Sites' root.tsx has.
  const router = createMemoryRouter(
    [
      {
        path: "/",
        ErrorBoundary: () => <h1>Something went wrong</h1>,
        children: [
          { path: "profile/:address", ...route },
          { path: "profile/:address/:tab", ...route },
          { path: "other", Component: Other },
        ],
      },
    ],
    { initialEntries: [path] },
  );
  render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return router;
}

/** The first document: the server's loader data is handed to the page as its props. */
function mountDocument(path: string, data: ProfileLoaderData) {
  const router = createMemoryRouter(
    [
      {
        path: "/profile/:address/:tab?",
        loader: () => data,
        Component: function Doc() {
          return <ProfileRoute loaderData={data} />;
        },
        ErrorBoundary: ProfileErrorBoundary,
      },
    ],
    { initialEntries: [path] },
  );
  render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return router;
}

function serverState(name: string): ProfileLoaderData {
  const server = new QueryClient();
  server.setQueryData(profileKeys.profile(A), mapProfile(avatar(name) as never, A));
  return { sid: "sid-1", address: A, own: false, dehydratedState: dehydrate(server) };
}

beforeEach(() => {
  client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: 30_000 } } });
  auth.state = { isConnected: false, address: null, identity: null };
});
afterEach(() => {
  cleanup();
  client.clear();
  vi.resetAllMocks();
});

describe("profile route, first hydration", () => {
  it("renders the server's profile without asking for it again", async () => {
    mountDocument(`/profile/${A}`, serverState("Alice"));

    expect(await screen.findByRole("heading", { name: "Alice" })).toBeTruthy();
    expect(net.fetchProfile).not.toHaveBeenCalled();
    expect(client.getQueryData(profileKeys.profile(A))).toMatchObject({ name: "Alice" });
  });

  it("keeps the profile on screen when a background re-read fails", async () => {
    mountDocument(`/profile/${A}`, serverState("Alice"));
    await screen.findByRole("heading", { name: "Alice" });
    net.fetchProfile.mockRejectedValue(new CatalystError("Catalyst unreachable", "/lambdas/profile", 0));

    await act(async () => {
      await client.refetchQueries({ queryKey: profileKeys.profile(A) });
    });

    expect(screen.getByRole("heading", { name: "Alice" })).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
  });
});

describe("profile route, client navigation", () => {
  it("receives no server data and reads the profile through the browser client", async () => {
    net.fetchProfile.mockResolvedValue(avatar("Alice"));

    mount(`/profile/${A}`);

    expect(await screen.findByRole("heading", { name: "Alice" })).toBeTruthy();
    expect(net.fetchProfile).toHaveBeenCalledTimes(1);
  });

  it("shows the unavailable screen when the profile cannot be read, and Retry reads it again", async () => {
    net.fetchProfile.mockRejectedValueOnce(new CatalystError("Catalyst unreachable", "/lambdas/profile", 0)).mockResolvedValueOnce(avatar("Alice"));

    mount(`/profile/${A}`);

    expect(await screen.findByRole("heading", { name: "Profile unavailable" })).toBeTruthy();
    await act(async () => {
      screen.getByRole("button", { name: "Try again" }).click();
    });
    // The retry is a navigation plus a fresh read; allow for a loaded test machine.
    expect(await screen.findByRole("heading", { name: "Alice" }, { timeout: 3000 })).toBeTruthy();
    expect(net.fetchProfile).toHaveBeenCalledTimes(2);
  });

  it("reads the profile afresh after leaving a failed profile and coming back", async () => {
    net.fetchProfile.mockRejectedValueOnce(new CatalystError("Catalyst unreachable", "/lambdas/profile", 0)).mockResolvedValueOnce(avatar("Alice"));
    const router = mount(`/profile/${A}`);
    await screen.findByRole("heading", { name: "Profile unavailable" });

    await act(async () => {
      await router.navigate("/other");
    });
    screen.getByRole("button", { name: "back to profile" }).click();

    expect(await screen.findByRole("heading", { name: "Alice" })).toBeTruthy();
    expect(net.fetchProfile).toHaveBeenCalledTimes(2);
  });

  it("hands a bug in the profile read to the root boundary, not the profile screen", async () => {
    net.fetchProfile.mockRejectedValue(new TypeError("Cannot read properties of undefined (reading 'name')"));

    mount(`/profile/${A}`);

    expect(await screen.findByRole("heading", { name: "Something went wrong" })).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Profile unavailable" })).toBeNull();
  });

  it("shows unavailable, not \"invalid address\", when the node answers 404 for a wallet", async () => {
    net.fetchProfile.mockRejectedValue(new CatalystError("Catalyst returned 404", "/lambdas/profile", 404));

    mount(`/profile/${A}`);

    expect(await screen.findByRole("heading", { name: "Profile unavailable" })).toBeTruthy();
  });

  it("shows not found for an address that cannot be a wallet, without reading anything", async () => {
    mount("/profile/not-a-wallet");

    expect(await screen.findByRole("heading", { name: "Profile not found" })).toBeTruthy();
    expect(net.fetchProfile).not.toHaveBeenCalled();
  });

  it("sends a member's hidden tab back to the profile root", async () => {
    net.fetchProfile.mockResolvedValue(avatar("Alice"));

    const router = mount(`/profile/${A}/assets`);

    await waitFor(() => expect(router.state.location.pathname).toBe(`/profile/${A}`));
  });
});

describe("profile route, places", () => {
  it("keeps the owner's empty state when a background profile re-read fails", async () => {
    auth.state = { isConnected: true, address: A, identity };
    net.fetchProfile.mockResolvedValueOnce(avatar("Alice")).mockRejectedValueOnce(new CatalystError("down", "/lambdas", 503));
    net.fetchProfilePlaces.mockResolvedValue({ places: [], total: 0 });
    mount(`/profile/${A}/places`);
    expect(await screen.findByText("Nothing here yet, but that's easy to change", { selector: "p" })).toBeTruthy();

    await act(async () => {
      await client.refetchQueries({ queryKey: profileKeys.profile(A) });
      // Query observers notify on the next tick; let the page render the failed re-read.
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    expect(client.getQueryState(profileKeys.profile(A))?.status).toBe("error");
    expect(screen.getByText("Nothing here yet, but that's easy to change", { selector: "p" })).toBeTruthy();
    expect(screen.queryByText("Loading places")).toBeNull();
  });
});

describe("profile route, creations and assets", () => {
  const hat = { id: "c-0", contractAddress: "0xabc", itemId: "0", name: "Billyjuice", network: "polygon", traits: { bodyShapes: [], isSmart: false } };

  it("shows a member's creations under their own tab", async () => {
    net.fetchProfile.mockResolvedValue(avatar("Alice"));
    net.fetchProfileCreations.mockResolvedValue({ items: [hat], received: 1, total: 1 });

    mount(`/profile/${A}/creations`);

    expect((await screen.findByRole("link", { name: "Billyjuice" })).getAttribute("href")).toBe("/shop/item/0xabc/0");
    expect(screen.getByText("By Alice")).toBeTruthy();
  });

  it("shows the owner's assets on their own profile", async () => {
    auth.state = { isConnected: true, address: A, identity };
    net.fetchProfile.mockResolvedValue(avatar("Alice"));
    net.fetchProfileAssets.mockResolvedValue({ items: [{ ...hat, id: "t-1", tokenId: "7", category: "wearable" }], received: 1, total: 1 });

    mount(`/profile/${A}/assets`);

    expect((await screen.findByRole("link", { name: "Billyjuice" })).getAttribute("href")).toBe("/shop/token/0xabc/7");
    expect(screen.getByText("1 asset")).toBeTruthy();
  });
});

describe("profile route, communities", () => {
  it("reports a failed read of the member's own communities once", async () => {
    auth.state = { isConnected: true, address: A, identity };
    net.fetchProfile.mockResolvedValue(avatar("Alice"));
    net.fetchMyCommunities.mockRejectedValue(new Error("500"));

    mount(`/profile/${A}/communities`);

    await waitFor(() =>
      expect(net.track).toHaveBeenCalledWith("profile_communities_load_failed", { address: A }, expect.anything()),
    );
    act(() => {
      focusManager.setFocused(false);
      focusManager.setFocused(true);
    });
    await new Promise((resolve) => setTimeout(resolve, 20));
    focusManager.setFocused(undefined);
    expect(net.track.mock.calls.filter(([event]) => event === "profile_communities_load_failed")).toHaveLength(1);
    expect(net.fetchMyCommunities).toHaveBeenCalledTimes(1);
  });

  it("neither reads nor reports communities on another member's profile", async () => {
    auth.state = { isConnected: true, address: "0x1111111111111111111111111111111111111111", identity };
    net.fetchProfile.mockResolvedValue(avatar("Alice"));

    mount(`/profile/${A}/communities`);

    expect(await screen.findByRole("heading", { name: "Alice" })).toBeTruthy();
    expect(net.fetchMyCommunities).not.toHaveBeenCalled();
    expect(net.track.mock.calls.some(([event]) => event === "profile_communities_load_failed")).toBe(false);
  });
});
