// @vitest-environment jsdom
import {
  dehydrate,
  focusManager,
  QueryClient,
  QueryClientProvider,
  QueryObserver,
} from "@tanstack/react-query";
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
  fetchMemberCommunities: vi.fn(),
  fetchProfilePlaces: vi.fn(),
  fetchProfileCreations: vi.fn(),
  fetchProfileAssets: vi.fn(),
  fetchUserPhotos: vi.fn(),
  track: vi.fn(),
}));
const auth = vi.hoisted(() => ({
  state: { isConnected: false, address: null as string | null, identity: null as unknown },
}));
vi.mock("@/data/lib/catalyst/social/profile", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/social/profile")>()),
  fetchProfile: net.fetchProfile,
}));
vi.mock("@/data/lib/catalyst/social/profile-communities", () => ({
  fetchMyCommunities: net.fetchMyCommunities,
  fetchMemberCommunities: net.fetchMemberCommunities,
}));
vi.mock("@/data/lib/catalyst/places/profile-places", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/places/profile-places")>()),
  fetchProfilePlaces: net.fetchProfilePlaces,
}));
vi.mock("@/data/lib/catalyst/marketplace/profile-collectibles", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/marketplace/profile-collectibles")>()),
  fetchProfileCreations: net.fetchProfileCreations,
  fetchProfileAssets: net.fetchProfileAssets,
  fetchProfileAssetCount: async () => 0,
}));
vi.mock("@/data/lib/catalyst/social/photos", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/social/photos")>()),
  fetchUserPhotos: net.fetchUserPhotos,
}));
vi.mock("@/data/lib/catalyst/social/badges", () => ({ fetchUserBadges: () => Promise.resolve([]) }));
vi.mock("@/core/lib/telemetry/track", () => ({ track: net.track }));
vi.mock("@/data/lib/auth/index", () => ({ useAuth: () => auth.state }));

import { CatalystError } from "@/data/lib/catalyst/client";
import { mapProfile, type ProfileVM } from "@/data/lib/catalyst/social/profile";
import { profileKeys, profileQuery } from "@/data/lib/profile/queries";

import type { ProfileLoaderData } from "./profile-loader.server";
import { ProfileErrorBoundary, ProfileRoute, profileClientLoader } from "./profile-route";

const A = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const avatar = (name: string) => ({ name, hasClaimedName: true, description: "", avatar: { snapshots: { face256: "" } } });
const B = "0x1111111111111111111111111111111111111111";
const identityFixture = () => ({ signer: A, ephemeral: { address: "0xeph", privateKey: "0xdeadbeef" }, expiration: "2999-01-01T00:00:00.000Z", authChain: [] });
const unreachable = () => new CatalystError("Catalyst unreachable", "/lambdas/profile", 0);

let client: QueryClient;
let identity: ReturnType<typeof identityFixture>;
let router: ReturnType<typeof createMemoryRouter>;

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
  return { sid: "sid-1", address: A, own: false, dehydratedState: dehydrate(server), card: {} };
}

/**
 * The unavailable screen, once the failed page no longer observes the profile read. React can show
 * the screen one task before it unsubscribes the page. A step taken in between acts on a read that
 * the page still owns: Retry reads through the page, and the page's exit cancels a read in flight.
 */
async function unavailable() {
  const heading = await screen.findByRole("heading", { name: "Profile unavailable" });
  await waitFor(() =>
    expect(client.getQueryCache().find({ queryKey: profileKeys.profile(A) })?.getObserversCount()).toBe(0),
  );
  return heading;
}

beforeEach(() => {
  client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: 30_000 } } });
  identity = identityFixture();
  auth.state = { isConnected: false, address: null, identity: null };
});
afterEach(() => {
  cleanup();
  client.clear();
  vi.resetAllMocks();
});

describe("profile route, first hydration", () => {
  beforeEach(() => {
    mountDocument(`/profile/${A}`, serverState("Alice"));
  });

  describe("when the server hands over the profile", () => {
    it("should render it", async () => {
      expect(await screen.findByRole("heading", { name: "Alice" })).toBeTruthy();
    });

    it("should not ask for it again", async () => {
      await screen.findByRole("heading", { name: "Alice" });
      expect(net.fetchProfile).not.toHaveBeenCalled();
    });

    it("should seed the browser cache with it", async () => {
      await screen.findByRole("heading", { name: "Alice" });
      expect(client.getQueryData(profileKeys.profile(A))).toMatchObject({ name: "Alice" });
    });
  });

  describe("when a background re-read fails", () => {
    beforeEach(async () => {
      await screen.findByRole("heading", { name: "Alice" });
      net.fetchProfile.mockRejectedValue(unreachable());

      await act(async () => {
        await client.refetchQueries({ queryKey: profileKeys.profile(A) });
      });
    });

    it("should keep the profile on screen", () => {
      expect(screen.getByRole("heading", { name: "Alice" })).toBeTruthy();
    });

    it("should show no error", () => {
      expect(screen.queryByRole("alert")).toBeNull();
    });
  });
});

describe("profile route, client navigation", () => {
  let fresh: ReturnType<typeof mapProfile>;

  beforeEach(() => {
    fresh = mapProfile(avatar("Recovered") as never, A);
  });

  describe("when the previous error boundary leaves and a newer healthy public profile is cached", () => {
    beforeEach(async () => {
      net.fetchProfile.mockRejectedValueOnce(unreachable());
      router = mount(`/profile/${A}`);
      await unavailable();
      act(() => client.setQueryData(profileKeys.profile(A), fresh));
    });

    it("should keep the newer profile and render it on return without reading again", async () => {
      await act(async () => {
        await router.navigate("/other");
      });
      expect(client.getQueryData(profileKeys.profile(A))).toEqual(fresh);
      await act(async () => {
        await router.navigate(`/profile/${A}`);
      });
      expect(await screen.findByRole("heading", { name: "Recovered" })).toBeTruthy();
      expect(net.fetchProfile).toHaveBeenCalledTimes(1);
    });
  });

  describe("when a newer background read fails after usable data was cached", () => {
    beforeEach(async () => {
      net.fetchProfile.mockRejectedValue(unreachable());
      router = mount(`/profile/${A}`);
      await unavailable();
      act(() => client.setQueryData(profileKeys.profile(A), fresh));
    });

    it("should reject the read and keep the cached data after leaving", async () => {
      await expect(
        client.fetchQuery({ ...profileQuery(A), staleTime: 0 }),
      ).rejects.toBeInstanceOf(CatalystError);
      await act(async () => {
        await router.navigate("/other");
      });
      expect(client.getQueryData(profileKeys.profile(A))).toEqual(fresh);
    });
  });

  describe("when the old boundary leaves while a newer public profile read is in flight", () => {
    let finish: (value: ReturnType<typeof avatar>) => void;
    let signal: AbortSignal | undefined;
    let read: Promise<{ ok: boolean; value?: unknown; error?: unknown }>;

    beforeEach(async () => {
      signal = undefined;
      const response = new Promise<ReturnType<typeof avatar>>((resolve) => {
        finish = resolve;
      });
      net.fetchProfile
        .mockRejectedValueOnce(unreachable())
        .mockImplementationOnce((_address, options: { signal: AbortSignal }) => {
          signal = options.signal;
          return response;
        });
      router = mount(`/profile/${A}`);
      await unavailable();
      read = client.fetchQuery(profileQuery(A)).then(
        (value) => ({ ok: true, value }),
        (error: unknown) => ({ ok: false, error }),
      );
    });

    it("should not cancel the read, which then resolves", async () => {
      try {
        await act(async () => {
          await router.navigate("/other");
        });
        expect(signal?.aborted).toBe(false);
      } finally {
        finish(avatar("Recovered"));
      }
      expect(await read).toMatchObject({
        ok: true,
        value: { name: "Recovered" },
      });
      expect(net.fetchProfile).toHaveBeenCalledTimes(2);
    });
  });

  describe("when another active observer owns a failed public profile", () => {
    let failure: CatalystError;
    let observer: QueryObserver<ProfileVM | null, Error, ProfileVM | null, ProfileVM | null, ReturnType<typeof profileKeys.profile>>;
    let unsubscribe: () => void;

    beforeEach(async () => {
      failure = unreachable();
      net.fetchProfile.mockRejectedValue(failure);
      router = mount(`/profile/${A}`);
      await unavailable();
      observer = new QueryObserver(client, {
        ...profileQuery(A),
        retryOnMount: false,
      });
      unsubscribe = observer.subscribe(() => {});
      await act(async () => {
        await router.navigate("/other");
      });
    });

    afterEach(() => {
      unsubscribe();
    });

    it("should leave the observer's error intact", () => {
      expect(observer.getCurrentResult().isError).toBe(true);
      expect(client.getQueryState(profileKeys.profile(A))?.error).toBe(failure);
    });

    it("should not read again", () => {
      expect(net.fetchProfile).toHaveBeenCalledTimes(1);
    });
  });

  describe("when Retry leaves the old error boundary and a newer healthy profile is cached", () => {
    beforeEach(async () => {
      net.fetchProfile.mockRejectedValueOnce(unreachable());
      mount(`/profile/${A}`);
      await unavailable();
      act(() => client.setQueryData(profileKeys.profile(A), fresh));

      await act(async () => {
        screen.getByRole("button", { name: "Try again" }).click();
      });
    });

    it("should use the newer profile", async () => {
      expect(await screen.findByRole("heading", { name: "Recovered" })).toBeTruthy();
    });

    it("should not read again", async () => {
      await screen.findByRole("heading", { name: "Recovered" });
      expect(net.fetchProfile).toHaveBeenCalledTimes(1);
    });
  });

  describe("when Retry fails repeatedly", () => {
    beforeEach(async () => {
      net.fetchProfile
        .mockRejectedValueOnce(unreachable())
        .mockRejectedValueOnce(unreachable())
        .mockResolvedValueOnce(avatar("Recovered"));
      router = mount(`/profile/${A}`);
      await unavailable();
    });

    it("should make one new read per attempt", async () => {
      await act(async () => {
        screen.getByRole("button", { name: "Try again" }).click();
      });
      await waitFor(() => expect(client.getQueryState(profileKeys.profile(A))?.status).toBe("error"));
      expect(router.state.navigation.state).toBe("idle");
      expect(net.fetchProfile).toHaveBeenCalledTimes(2);
      expect(await unavailable()).toBeTruthy();

      await act(async () => {
        screen.getByRole("button", { name: "Try again" }).click();
      });
      expect(await screen.findByRole("heading", { name: "Recovered" })).toBeTruthy();
      expect(net.fetchProfile).toHaveBeenCalledTimes(3);
    });
  });

  describe("when the profile is read on a client navigation", () => {
    beforeEach(() => {
      net.fetchProfile.mockResolvedValue(avatar("Alice"));

      mount(`/profile/${A}`);
    });

    it("should render it from the browser client's read, with no server data", async () => {
      expect(await screen.findByRole("heading", { name: "Alice" })).toBeTruthy();
    });

    it("should read it once", async () => {
      await screen.findByRole("heading", { name: "Alice" });
      expect(net.fetchProfile).toHaveBeenCalledTimes(1);
    });
  });

  describe("when the profile cannot be read", () => {
    beforeEach(() => {
      net.fetchProfile.mockRejectedValueOnce(unreachable()).mockResolvedValueOnce(avatar("Alice"));
    });

    describe("and the visitor stays on the profile", () => {
      beforeEach(() => {
        mount(`/profile/${A}`);
      });

      it("should show the unavailable screen", async () => {
        expect(await unavailable()).toBeTruthy();
      });

      describe("and the visitor clicks Retry", () => {
        beforeEach(async () => {
          await unavailable();
          await act(async () => {
            screen.getByRole("button", { name: "Try again" }).click();
          });
        });

        it("should read it again and render it", async () => {
          // The retry is a navigation plus a fresh read; allow for a loaded test machine.
          expect(await screen.findByRole("heading", { name: "Alice" }, { timeout: 3000 })).toBeTruthy();
          expect(net.fetchProfile).toHaveBeenCalledTimes(2);
        });
      });
    });

    describe("and the visitor leaves", () => {
      beforeEach(async () => {
        router = mount(`/profile/${A}`);
        await unavailable();

        await act(async () => {
          await router.navigate("/other");
        });
      });

      it("should reset the failed read", () => {
        expect(client.getQueryState(profileKeys.profile(A))).toMatchObject({
          status: "pending",
          error: null,
          fetchStatus: "idle",
        });
      });

      describe("and comes back", () => {
        beforeEach(() => {
          screen.getByRole("button", { name: "back to profile" }).click();
        });

        it("should read the profile afresh", async () => {
          expect(await screen.findByRole("heading", { name: "Alice" })).toBeTruthy();
          expect(net.fetchProfile).toHaveBeenCalledTimes(2);
        });
      });
    });
  });

  describe("when the profile read has a bug", () => {
    beforeEach(() => {
      net.fetchProfile.mockRejectedValue(new TypeError("Cannot read properties of undefined (reading 'name')"));

      mount(`/profile/${A}`);
    });

    it("should hand it to the root boundary", async () => {
      expect(await screen.findByRole("heading", { name: "Something went wrong" })).toBeTruthy();
    });

    it("should not show the profile screen", async () => {
      await screen.findByRole("heading", { name: "Something went wrong" });
      expect(screen.queryByRole("heading", { name: "Profile unavailable" })).toBeNull();
    });
  });

  describe("when the node answers 404 for a wallet", () => {
    beforeEach(() => {
      net.fetchProfile.mockRejectedValue(new CatalystError("Catalyst returned 404", "/lambdas/profile", 404));

      mount(`/profile/${A}`);
    });

    it("should show unavailable, not \"invalid address\"", async () => {
      expect(await unavailable()).toBeTruthy();
    });
  });

  describe("when the address cannot be a wallet", () => {
    beforeEach(() => {
      mount("/profile/not-a-wallet");
    });

    it("should show not found", async () => {
      expect(await screen.findByRole("heading", { name: "Profile not found" })).toBeTruthy();
    });

    it("should not read anything", async () => {
      await screen.findByRole("heading", { name: "Profile not found" });
      expect(net.fetchProfile).not.toHaveBeenCalled();
    });
  });

  describe("when a member's hidden tab is opened", () => {
    beforeEach(() => {
      net.fetchProfile.mockResolvedValue(avatar("Alice"));

      router = mount(`/profile/${A}/assets`);
    });

    it("should send it back to the profile root", async () => {
      await waitFor(() => expect(router.state.location.pathname).toBe(`/profile/${A}`));
    });
  });
});

describe("profile route, places", () => {
  describe("when the owner's places are empty and a background profile re-read fails", () => {
    beforeEach(async () => {
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
    });

    it("should record the failed re-read", () => {
      expect(client.getQueryState(profileKeys.profile(A))?.status).toBe("error");
    });

    it("should keep the owner's empty state", () => {
      expect(screen.getByText("Nothing here yet, but that's easy to change", { selector: "p" })).toBeTruthy();
      expect(screen.queryByText("Loading places")).toBeNull();
    });
  });
});

describe("profile route, creations and assets", () => {
  let hat: { id: string; contractAddress: string; itemId: string; name: string; network: string; payment: { kind: string }; traits: { bodyShapes: never[]; isSmart: boolean } };

  beforeEach(() => {
    hat = { id: "c-0", contractAddress: "0xabc", itemId: "0", name: "Billyjuice", network: "polygon", payment: { kind: "none" }, traits: { bodyShapes: [], isSmart: false } };
  });

  describe("when a member's creations are opened", () => {
    beforeEach(() => {
      net.fetchProfile.mockResolvedValue(avatar("Alice"));
      net.fetchProfileCreations.mockResolvedValue({ items: [hat], received: 1, total: 1 });

      router = mount(`/profile/${A}/creations`);
    });

    it("should link each creation to its shop item", async () => {
      expect((await screen.findByRole("link", { name: "Billyjuice" })).getAttribute("href")).toBe("/shop/item/0xabc/0");
    });

    it("should show them under the member's own tab", async () => {
      await screen.findByRole("link", { name: "Billyjuice" });
      expect(screen.getByText("Alice", { selector: "article *" })).toBeTruthy();
    });

    describe("and the visitor picks a category and then opens another profile's creations", () => {
      beforeEach(async () => {
        await screen.findByRole("link", { name: "Billyjuice" });
        await act(async () => screen.getByRole("tab", { name: "Emotes" }).click());
      });

      it("should open the other profile on the default category", async () => {
        expect(screen.getByRole("tab", { name: "Emotes" }).getAttribute("aria-selected")).toBe("true");

        await act(async () => router.navigate(`/profile/${B}/creations`));

        await waitFor(() => expect(screen.getByRole("tab", { name: "Wearables" }).getAttribute("aria-selected")).toBe("true"));
        expect(net.fetchProfileCreations).toHaveBeenLastCalledWith(B, expect.objectContaining({ category: "wearable" }), expect.anything());
      });
    });
  });

  describe("when the owner opens their own assets", () => {
    beforeEach(() => {
      auth.state = { isConnected: true, address: A, identity };
      net.fetchProfile.mockResolvedValue(avatar("Alice"));
      net.fetchProfileAssets.mockResolvedValue({ items: [{ ...hat, id: "t-1", tokenId: "7", category: "wearable" }], received: 1, total: 1 });

      mount(`/profile/${A}/assets`);
    });

    it("should link each asset to its shop token", async () => {
      expect((await screen.findByRole("link", { name: "Billyjuice" })).getAttribute("href")).toBe("/shop/token/0xabc/7");
    });

    it("should count the assets", async () => {
      await screen.findByRole("link", { name: "Billyjuice" });
      expect(screen.getByText("1 asset")).toBeTruthy();
    });
  });
});

describe("profile route, photos", () => {
  describe("when a member's photos are opened with a link naming one", () => {
    beforeEach(() => {
      const shot = {
        id: "p-1",
        url: "https://reel.example/p-1.jpg",
        thumbnailUrl: "https://reel.example/p-1-thumbnail.jpg",
        isPublic: true,
        dateTime: "",
        realm: "main",
        scene: { name: "Genesis Plaza", x: "0", y: "1" },
        author: { name: "Alice", address: A },
        people: [],
      };
      net.fetchProfile.mockResolvedValue(avatar("Alice"));
      net.fetchUserPhotos.mockResolvedValue({ items: [shot], received: 1, total: 1 });

      mount(`/profile/${A}/photos?photo=p-1`);
    });

    it("should open the named photo", async () => {
      expect(await screen.findByRole("dialog", { name: "Photo at Genesis Plaza, 1 of 1" })).toBeTruthy();
    });

    it("should show the member's photos", async () => {
      await screen.findByRole("dialog", { name: "Photo at Genesis Plaza, 1 of 1" });
      expect(screen.getByRole("button", { name: "Photo at Genesis Plaza" })).toBeTruthy();
      expect(screen.getByText("1 photo")).toBeTruthy();
    });
  });
});

describe("profile route, communities", () => {
  describe("when the read of the member's own communities fails", () => {
    beforeEach(async () => {
      auth.state = { isConnected: true, address: A, identity };
      net.fetchProfile.mockResolvedValue(avatar("Alice"));
      net.fetchMyCommunities.mockRejectedValue(new Error("500"));

      mount(`/profile/${A}/communities`);

      await waitFor(() =>
        expect(net.track).toHaveBeenCalledWith("profile_communities_load_failed", { address: A }, expect.anything()),
      );
    });

    describe("and the window regains focus", () => {
      beforeEach(async () => {
        act(() => {
          focusManager.setFocused(false);
          focusManager.setFocused(true);
        });
        await new Promise((resolve) => setTimeout(resolve, 20));
        focusManager.setFocused(undefined);
      });

      it("should report the failure once", () => {
        expect(net.track.mock.calls.filter(([event]) => event === "profile_communities_load_failed")).toHaveLength(1);
      });

      it("should not read again", () => {
        expect(net.fetchMyCommunities).toHaveBeenCalledTimes(1);
      });
    });
  });

  describe("when another member's communities are opened", () => {
    beforeEach(() => {
      auth.state = { isConnected: true, address: B, identity };
      net.fetchProfile.mockResolvedValue(avatar("Alice"));
      net.fetchMemberCommunities.mockResolvedValue({
        communities: [{ id: "c1", name: "Builders", role: "owner", membersCount: null, privacy: null, thumbnailUrl: null }],
        total: 1,
      });

      mount(`/profile/${A}/communities`);
    });

    it("should list their public communities", async () => {
      expect((await screen.findByRole("link", { name: /Builders/ })).getAttribute("href")).toBe("/social/communities/c1");
    });

    it("should list them in the profile's frame", async () => {
      await screen.findByRole("link", { name: /Builders/ });
      expect(screen.getByRole("tab", { name: "Communities" }).getAttribute("aria-selected")).toBe("true");
    });

    it("should read them unsigned", async () => {
      await screen.findByRole("link", { name: /Builders/ });
      expect(net.fetchMemberCommunities).toHaveBeenCalledWith(A, expect.anything());
      expect(net.fetchMyCommunities).not.toHaveBeenCalled();
    });

    it("should report no failure", async () => {
      await screen.findByRole("link", { name: /Builders/ });
      expect(net.track.mock.calls.some(([event]) => event === "profile_communities_load_failed")).toBe(false);
    });
  });
});
