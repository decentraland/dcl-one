// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { type ComponentProps, StrictMode } from "react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});
const net = vi.hoisted(() => ({ fetchUserPhotos: vi.fn(), fetchPhoto: vi.fn(), fetchCommunityProfiles: vi.fn() }));
const auth = vi.hoisted(() => {
  type State = { identity: unknown; isConnected: boolean; address: string | null };
  const listeners = new Set<() => void>();
  const store = {
    state: { identity: null, isConnected: false, address: null } as State,
    /** Changes the signed-in state of a mounted page, as signing in in the browser does. */
    set(next: State) {
      store.state = next;
      for (const listener of listeners) listener();
    },
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
  return store;
});
vi.mock("@/data/lib/catalyst/social/photos", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/social/photos")>()),
  fetchUserPhotos: net.fetchUserPhotos,
  fetchPhoto: net.fetchPhoto,
}));
vi.mock("@ui/data/hooks/useCommunityProfiles", () => ({ fetchCommunityProfiles: net.fetchCommunityProfiles }));
vi.mock("@/data/lib/auth/index", async () => {
  const { useSyncExternalStore } = await import("react");
  return { useAuth: () => useSyncExternalStore(auth.subscribe, () => auth.state) };
});

import { playUrl } from "@/data/lib/catalyst/bases";
import { formatPhotoDate, type ProfilePhoto } from "@/data/lib/catalyst/social/photos";
import { emptyProfile, type ProfileVM } from "@/data/lib/catalyst/social/profile";
import { PROFILE_COLLECTIBLES_PAGE, profileKeys } from "@/data/lib/profile/queries";

import { ProfilePhotosScreen } from "./ProfilePhotosScreen";

const A = "0xe5104e449950180c459447f2971c8a760456b635";
const FRIEND = "0x09bc69a986e1b37fb01ef36124f4c2e14fa0ab6b";
let identity: { signer: string; ephemeral: { address: string; privateKey: string }; expiration: string; authChain: never[] };
let tabs: ComponentProps<typeof ProfilePhotosScreen>["tabs"];
const DATE = "1794327859";
const date = formatPhotoDate(DATE);

function photo(n: number, over: Partial<ProfilePhoto> = {}): ProfilePhoto {
  return {
    id: `p-${n}`,
    url: `https://reel.example/p-${n}.jpg`,
    thumbnailUrl: `https://reel.example/p-${n}-thumbnail.jpg`,
    isPublic: true,
    dateTime: DATE,
    realm: "main",
    scene: { name: `Plaza ${n}`, x: "0", y: "1" },
    author: { name: "kurd", address: A },
    people: [],
    ...over,
  };
}

const page = (items: ProfilePhoto[], total = items.length) => ({ items, received: items.length, total });
const tileName = (n: number) => `Photo at Plaza ${n}, ${date}`;

function profile(over: Partial<ProfileVM> = {}): ProfileVM {
  return { ...emptyProfile(A), name: "kurd", ...over };
}

function show({ own = false, path = `/profile/${A}/photos`, strict = false } = {}) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter(
    [{ path: "*", element: <ProfilePhotosScreen profile={profile()} own={own} tabs={tabs} onTab={vi.fn()} /> }],
    { initialEntries: [path] },
  );
  const tree = (
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  );
  render(strict ? <StrictMode>{tree}</StrictMode> : tree);
  return { client, router };
}

type Router = ReturnType<typeof show>["router"];

const tile = (n: number) => screen.findByRole("button", { name: tileName(n) });
/** An id in the service's UUID format, for photos a link reads by id. */
const uuid = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, "0")}`;

let user: ReturnType<typeof userEvent.setup>;

beforeEach(() => {
  identity = { signer: A, ephemeral: { address: "0xeph", privateKey: "0x01" }, expiration: "2999-01-01T00:00:00.000Z", authChain: [] };
  tabs = [
    { id: "overview", label: "Overview", href: `/profile/${A}` },
    { id: "photos", label: "Photos", href: `/profile/${A}/photos` },
  ];
  auth.state = { identity: null, isConnected: false, address: null };
  net.fetchCommunityProfiles.mockResolvedValue({});
  user = userEvent.setup();
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  vi.restoreAllMocks();
  document.documentElement.style.overflow = "";
});

describe("ProfilePhotosScreen", () => {
  describe("when a visitor views a member's grid", () => {
    describe("and the member has more photos than one page", () => {
      beforeEach(() => {
        net.fetchUserPhotos.mockResolvedValue(page([photo(0), photo(1)], 30));
        show();
      });

      it("should show the member's public photos, a page at a time, read unsigned", async () => {
        const first = await tile(0);
        expect(first.querySelector("img")?.getAttribute("src")).toBe("https://reel.example/p-0-thumbnail.jpg");
        expect(await tile(1)).toBeTruthy();
        expect(screen.getByText("Showing 2 of 30 photos")).toBeTruthy();
        expect(net.fetchUserPhotos).toHaveBeenCalledWith(A, { limit: PROFILE_COLLECTIBLES_PAGE, offset: 0 }, { signal: expect.any(AbortSignal) });
        expect(screen.queryByRole("tablist", { name: "Photos shown" })).toBeNull();
      });
    });

    describe("and a thumbnail fails to load", () => {
      let first: HTMLElement;

      beforeEach(async () => {
        net.fetchUserPhotos.mockResolvedValue(page([photo(0)]));
        show();
        first = await tile(0);

        act(() => {
          first.querySelector("img")!.dispatchEvent(new Event("error"));
        });
      });

      it("should drop the broken thumbnail", () => {
        expect(first.querySelector("img")).toBeNull();
      });

      it("should keep a tile that opens the viewer", async () => {
        await user.click(first);
        expect(screen.getByRole("dialog", { name: `${tileName(0)}, 1 of 1` })).toBeTruthy();
      });
    });

    describe("and a thumbnail failed before the page hydrated", () => {
      beforeEach(() => {
        // A server-rendered image can fail before onError is attached: it is complete with no size.
        vi.spyOn(HTMLImageElement.prototype, "complete", "get").mockReturnValue(true);
        vi.spyOn(HTMLImageElement.prototype, "naturalWidth", "get").mockReturnValue(0);
        net.fetchUserPhotos.mockResolvedValue(page([photo(0)]));
        show();
      });

      it("should show the placeholder", async () => {
        const first = await tile(0);
        await waitFor(() => expect(first.querySelector("img")).toBeNull());
      });
    });

    describe("and the member has one photo", () => {
      beforeEach(async () => {
        net.fetchUserPhotos.mockResolvedValue(page([photo(0)]));
        show();
        await tile(0);
      });

      it("should show no visibility tabs, only the profile frame's own tabs", () => {
        // Only the profile frame's own tabs and tabpanel: no visibility switch inside it.
        expect(screen.queryByRole("tablist", { name: "Photos shown" })).toBeNull();
      });

      it("should show no visibility panel role, only the profile frame's own tabpanel", () => {
        expect(screen.getAllByRole("tabpanel")).toHaveLength(1);
      });
    });

    describe("and the member has taken no photos", () => {
      beforeEach(() => {
        net.fetchUserPhotos.mockResolvedValue(page([]));
        show();
      });

      it("should tell the visitor the member has taken no photos", async () => {
        expect(await screen.findByText("This member has not taken any photos yet.")).toBeTruthy();
        expect(screen.queryByText("No photos yet")).toBeNull();
      });
    });

    describe("and the read fails once", () => {
      beforeEach(() => {
        net.fetchUserPhotos.mockRejectedValueOnce(new Error("503")).mockResolvedValueOnce(page([photo(0)]));
        show();
      });

      it("should say the read failed, never an empty gallery, and try again", async () => {
        expect(await screen.findByText("Photos are unavailable right now.")).toBeTruthy();
        expect(screen.queryByText("This member has not taken any photos yet.")).toBeNull();
        await user.click(screen.getByRole("button", { name: "Try again" }));

        expect(await tile(0)).toBeTruthy();
      });
    });

    describe("and a background re-read fails", () => {
      let client: QueryClient;

      beforeEach(async () => {
        net.fetchUserPhotos.mockResolvedValueOnce(page([photo(0)])).mockRejectedValueOnce(new Error("503"));
        ({ client } = show());
        await tile(0);

        await act(async () => {
          await client.refetchQueries({ queryKey: profileKeys.photos(A) });
          await new Promise((resolve) => setTimeout(resolve, 0));
        });
      });

      it("should keep the photos on screen", () => {
        expect(client.getQueryState(profileKeys.photos(A))?.status).toBe("error");
        expect(screen.getByRole("button", { name: tileName(0) })).toBeTruthy();
        expect(screen.queryByText("Photos are unavailable right now.")).toBeNull();
      });
    });

    describe("and the next page fails once", () => {
      beforeEach(async () => {
        const first = Array.from({ length: PROFILE_COLLECTIBLES_PAGE }, (_, n) => photo(n));
        net.fetchUserPhotos
          .mockResolvedValueOnce(page(first, 30))
          .mockRejectedValueOnce(new Error("503"))
          .mockResolvedValueOnce(page([photo(24)], 30));
        show();
        await tile(0);
      });

      it("should load more after the photos shown and keep them", async () => {
        await user.click(screen.getByRole("button", { name: "Load more" }));

        expect(await screen.findByText("More photos could not be loaded.")).toBeTruthy();
        expect(screen.getByRole("button", { name: tileName(23) })).toBeTruthy();
        await user.click(screen.getByRole("button", { name: "Try again" }));
        expect(await tile(24)).toBeTruthy();
        expect(net.fetchUserPhotos.mock.calls[2]?.[1]).toEqual({ limit: PROFILE_COLLECTIBLES_PAGE, offset: PROFILE_COLLECTIBLES_PAGE });
        expect(screen.getByText("Showing 25 of 30 photos")).toBeTruthy();
      });
    });
  });

  describe("when a signed-in visitor views another member's photos", () => {
    beforeEach(() => {
      auth.state = { identity: { ...identity, signer: FRIEND }, isConnected: true, address: FRIEND };
      net.fetchUserPhotos.mockResolvedValue(page([photo(0)]));
      show({ own: false });
    });

    it("should never sign the read with the visitor's own identity", async () => {
      expect(await tile(0)).toBeTruthy();
      expect(net.fetchUserPhotos.mock.calls[0]?.[2]).not.toHaveProperty("identity");
    });
  });

  describe("when the owner views their photos", () => {
    beforeEach(() => {
      auth.state = { identity, isConnected: true, address: A };
    });

    describe("and the owner has no photos", () => {
      beforeEach(() => {
        net.fetchUserPhotos.mockResolvedValue(page([]));
        show({ own: true });
      });

      it("should invite the owner to take their first photo in-world", async () => {
        const title = await screen.findByText("No photos yet", { selector: "p" });
        expect(within(title.parentElement!).getByRole("link", { name: /Jump in/ }).getAttribute("href")).toBe(playUrl());
      });
    });

    describe("and the gallery holds a public and a private photo", () => {
      let client: QueryClient;

      beforeEach(() => {
        net.fetchUserPhotos.mockResolvedValue(page([photo(0), photo(1, { isPublic: false })], 2));
        ({ client } = show({ own: true }));
      });

      it("should read the owner's gallery signed and open on the public photos", async () => {
        expect(await tile(0)).toBeTruthy();
        expect(screen.queryByRole("button", { name: `${tileName(1)}, private` })).toBeNull();
        expect(net.fetchUserPhotos.mock.calls[0]?.[2]).toMatchObject({ identity });
        expect(client.getQueryData(profileKeys.myPhotos(A))).toBeDefined();
        expect(client.getQueryData(profileKeys.photos(A))).toBeUndefined();
        expect(screen.getByRole("tab", { name: "Public" }).getAttribute("aria-selected")).toBe("true");
      });

      describe("and the owner picks Private", () => {
        beforeEach(async () => {
          await tile(0);
          await user.click(screen.getByRole("tab", { name: "Private" }));
        });

        it("should show the private photos, marked", () => {
          expect(screen.getByRole("button", { name: `${tileName(1)}, private` })).toBeTruthy();
          expect(screen.queryByRole("button", { name: tileName(0) })).toBeNull();
          expect(screen.getByText("1 photo")).toBeTruthy();
        });

        describe("and then picks All", () => {
          beforeEach(async () => {
            await user.click(screen.getByRole("tab", { name: "All" }));
          });

          it("should show every photo", () => {
            expect(screen.getByRole("button", { name: tileName(0) })).toBeTruthy();
            expect(screen.getByRole("button", { name: `${tileName(1)}, private` })).toBeTruthy();
            expect(screen.getByText("2 photos")).toBeTruthy();
          });
        });
      });

      describe("and the owner uses the arrow keys on the visibility tabs", () => {
        let publicTab: HTMLElement;

        beforeEach(async () => {
          await tile(0);
          publicTab = screen.getByRole("tab", { name: "Public" });
        });

        it("should switch the visibility and label the panel by the selected tab", async () => {
          // The visibility panel, inside the profile frame's own tabpanel.
          expect(screen.getByRole("tabpanel", { name: "Public" })).toBeTruthy();
          const panel = document.getElementById(publicTab.getAttribute("aria-controls") ?? "");
          expect(panel?.getAttribute("role")).toBe("tabpanel");
          expect(panel?.getAttribute("aria-labelledby")).toBe(publicTab.id);

          publicTab.focus();
          await user.keyboard("{ArrowRight}");

          expect(screen.getByRole("tab", { name: "Private" }).getAttribute("aria-selected")).toBe("true");
          expect(screen.getByRole("tabpanel", { name: "Private" })).toBeTruthy();
          expect(screen.getByRole("button", { name: `${tileName(1)}, private` })).toBeTruthy();
          await user.keyboard("{Home}");
          expect(screen.getByRole("tab", { name: "All" }).getAttribute("aria-selected")).toBe("true");
        });
      });
    });

    describe("and no loaded photo matches the Private filter", () => {
      beforeEach(async () => {
        net.fetchUserPhotos.mockResolvedValue(page([photo(0)], 40));
        show({ own: true });
        await tile(0);
        await user.click(screen.getByRole("tab", { name: "Private" }));
      });

      it("should say no photo matches", () => {
        expect(screen.getByText("No photos match this filter.")).toBeTruthy();
      });

      it("should still offer the next page", () => {
        expect(screen.getByRole("button", { name: "Load more" })).toBeTruthy();
      });
    });

    describe("and the owner's identity arrives after the public read", () => {
      let release: (value: unknown) => void;

      beforeEach(() => {
        // The page first reads the public list (no identity yet), then the owner's identity arrives.
        auth.state = { identity: null, isConnected: true, address: A };
        net.fetchUserPhotos.mockImplementation((_a: string, _p: unknown, opts: { identity?: unknown }) =>
          opts.identity ? new Promise((resolve) => (release = resolve)) : Promise.resolve(page([])),
        );
        show({ own: true });
      });

      it("should wait for the signed list instead of saying the owner has no photos", async () => {
        expect(await screen.findByText("No photos yet", { selector: "p" })).toBeTruthy();

        act(() => auth.set({ identity, isConnected: true, address: A }));

        expect(screen.queryByText("No photos yet", { selector: "p" })).toBeNull();
        expect(screen.getByText("Loading photos")).toBeTruthy();
        await act(async () => release(page([photo(1, { isPublic: false })])));
        await user.click(await screen.findByRole("tab", { name: "Private" }));
        expect(screen.getByRole("button", { name: `${tileName(1)}, private` })).toBeTruthy();
      });
    });

    describe("and the signed read fails", () => {
      beforeEach(() => {
        auth.state = { identity: null, isConnected: true, address: A };
      });

      describe("and the public read has one photo", () => {
        beforeEach(async () => {
          net.fetchUserPhotos.mockImplementation(async (_a: string, _p: unknown, opts: { identity?: unknown }) => {
            if (opts.identity) throw new Error("401");
            return page([photo(0)]);
          });
          show({ own: true });
          await tile(0);

          act(() => auth.set({ identity, isConnected: true, address: A }));
        });

        it("should keep the public grid, without the filter, and try again", async () => {
          expect(await screen.findByText("Your private photos could not be loaded.", { exact: false })).toBeTruthy();
          expect(screen.getByRole("button", { name: tileName(0) })).toBeTruthy();
          expect(screen.queryByRole("tablist", { name: "Photos shown" })).toBeNull();
          expect(screen.queryByText("Photos are unavailable right now.")).toBeNull();
          net.fetchUserPhotos.mockResolvedValue(page([photo(0), photo(1, { isPublic: false })]));
          await user.click(screen.getByRole("button", { name: "Try again" }));
          expect(await screen.findByRole("tablist", { name: "Photos shown" })).toBeTruthy();
        });
      });

      describe("and a link names a public photo past the loaded pages", () => {
        let router: Router;

        beforeEach(async () => {
          net.fetchUserPhotos.mockImplementation(async (_a: string, _p: unknown, opts: { identity?: unknown }) => {
            if (opts.identity) throw new Error("401");
            return page([photo(0)], 40);
          });
          net.fetchPhoto.mockResolvedValue(photo(31, { id: uuid(31) }));
          ({ router } = show({ own: true, path: `/profile/${A}/photos?photo=${uuid(31)}` }));
          await tile(0);

          act(() => auth.set({ identity, isConnected: true, address: A }));
        });

        it("should read the link and open it, since it is public", async () => {
          expect(await screen.findByRole("dialog", { name: `${tileName(31)}, 1 of 1` })).toBeTruthy();
          expect(router.state.location.search).toBe(`?photo=${uuid(31)}`);
        });
      });
    });

    describe("and the owner has no identity in this browser", () => {
      beforeEach(() => {
        auth.state = { identity: null, isConnected: true, address: A };
        net.fetchUserPhotos.mockResolvedValue(page([photo(0)]));
        show({ own: true });
      });

      it("should read the public gallery, without the filter", async () => {
        expect(await tile(0)).toBeTruthy();
        expect(net.fetchUserPhotos.mock.calls[0]?.[2]).not.toHaveProperty("identity");
        expect(screen.queryByRole("tablist", { name: "Photos shown" })).toBeNull();
      });
    });

    describe("and the owner's link names their own private photo past the pages loaded", () => {
      beforeEach(() => {
        net.fetchUserPhotos.mockResolvedValue(page([photo(0)], 40));
        net.fetchPhoto.mockResolvedValue(photo(30, { id: uuid(30), isPublic: false }));
        show({ own: true, path: `/profile/${A}/photos?photo=${uuid(30)}` });
      });

      it("should open it", async () => {
        expect(await screen.findByRole("dialog", { name: `${tileName(30)}, 1 of 1` })).toBeTruthy();
      });
    });

    describe("and the owner's link names their own private photo in the loaded pages", () => {
      beforeEach(() => {
        net.fetchUserPhotos.mockResolvedValue(page([photo(0), photo(1, { isPublic: false })]));
        show({ own: true, path: `/profile/${A}/photos?photo=p-1` });
      });

      it("should open it, showing every photo", async () => {
        expect(await screen.findByRole("dialog", { name: `${tileName(1)}, 2 of 2` })).toBeTruthy();
        expect(screen.getByRole("tab", { name: "All" }).getAttribute("aria-selected")).toBe("true");
        expect(net.fetchPhoto).not.toHaveBeenCalled();
      });
    });
  });

  describe("when a visitor opens a photo in the viewer", () => {
    describe("and the photo has people in it", () => {
      let people: ProfilePhoto["people"];
      let router: Router;

      beforeEach(() => {
        people = [
          { name: "Alice", address: FRIEND, isGuest: false },
          { name: "", address: "0x2222222222222222222222222222222222222222", isGuest: true },
          { name: "Ghost", address: "not-a-wallet", isGuest: true },
        ];
        net.fetchCommunityProfiles.mockResolvedValue({ [FRIEND]: { name: "Alice", picture: "https://faces.example/alice.png" } });
      });

      describe("and the gallery holds two photos", () => {
        beforeEach(async () => {
          net.fetchUserPhotos.mockResolvedValue(page([photo(0, { people }), photo(1)]));
          ({ router } = show());
          await user.click(await tile(0));
        });

        it("should show its date, author, place and the people in it", async () => {
          const dialog = screen.getByRole("dialog", { name: `${tileName(0)}, 1 of 2` });
          expect(router.state.location.search).toBe("?photo=p-0");
          expect(within(dialog).getByRole("img", { name: "Plaza 0" }).getAttribute("src")).toBe("https://reel.example/p-0.jpg");
          expect(within(dialog).getByText(date)).toBeTruthy();
          expect(within(dialog).getByRole("link", { name: "kurd" }).getAttribute("href")).toBe(`/profile/${A}`);
          expect(within(dialog).getByText("Plaza 0 0,1")).toBeTruthy();
          expect(within(dialog).getByRole("link", { name: "Jump in to Plaza 0 0,1" }).getAttribute("href")).toBe(playUrl({ position: "0,1" }));
          expect(within(dialog).getByRole("link", { name: "Alice" }).getAttribute("href")).toBe(`/profile/${FRIEND}`);
          expect(within(dialog).getByRole("link", { name: "0x2222…2222" })).toBeTruthy();
          // A person without a readable wallet is named, not linked.
          expect(within(dialog).getByText("Ghost").tagName).toBe("SPAN");
          expect(within(dialog).getAllByText("guest")).toHaveLength(2);
          expect(within(dialog).getByRole("link", { name: "Download photo" }).getAttribute("href")).toBe("https://reel.example/p-0.jpg");
          await waitFor(() => expect(within(dialog).getByRole("img", { name: "Alice", hidden: true }).getAttribute("src")).toBe("https://faces.example/alice.png"));
          expect(net.fetchCommunityProfiles.mock.calls[0]?.[0]).toEqual([FRIEND, "0x2222222222222222222222222222222222222222"].sort());
        });
      });

      describe("and a person's face is missing or fails to load", () => {
        beforeEach(async () => {
          net.fetchUserPhotos.mockResolvedValue(page([photo(0, { people: people.slice(0, 2) })]));
          show();
          await user.click(await tile(0));
        });

        it("should show the person's initial", async () => {
          const dialog = screen.getByRole("dialog");
          const face = await within(dialog).findByRole("img", { name: "Alice", hidden: true });
          expect(within(dialog).getByText("0")).toBeTruthy();

          act(() => {
            face.dispatchEvent(new Event("error"));
          });

          expect(within(dialog).queryByRole("img", { name: "Alice", hidden: true })).toBeNull();
          expect(within(dialog).getByText("A")).toBeTruthy();
        });
      });
    });

    describe("and the photos were taken in a world, without coordinates and on an uploaded realm URL", () => {
      beforeEach(async () => {
        net.fetchUserPhotos.mockResolvedValue(
          page([
            photo(0, { realm: "sample.dcl.eth", scene: { name: "World", x: "3", y: "-4" } }),
            photo(1, { scene: { name: "Nowhere", x: "", y: "" } }),
            photo(2, { realm: "https://peer.example", scene: { name: "Server", x: "5", y: "5" } }),
          ]),
        );
        show();
        await user.click(await screen.findByRole("button", { name: `Photo at World, ${date}` }));
      });

      it("should jump in to the world, never to an uploaded realm URL, and offer no Jump In without coordinates", async () => {
        expect(screen.getByRole("link", { name: "Jump in to World 3,-4" }).getAttribute("href")).toBe(
          playUrl({ position: "3,-4", realm: "sample.dcl.eth" }),
        );
        await user.keyboard("{ArrowRight}");

        const dialog = screen.getByRole("dialog", { name: `Photo at Nowhere, ${date}, 2 of 3` });
        expect(within(dialog).queryByRole("link", { name: /Jump in/ })).toBeNull();
        await user.keyboard("{ArrowRight}");
        // An uploaded realm URL is never forwarded: it could send the Explorer to the uploader's server.
        const server = screen.getByRole("link", { name: "Jump in to Server 5,5" }).getAttribute("href")!;
        expect(server).toBe(playUrl({ position: "5,5" }));
        expect(server).not.toContain("peer.example");
      });
    });

    describe("and the gallery holds three photos", () => {
      let router: Router;

      beforeEach(async () => {
        net.fetchUserPhotos.mockResolvedValue(page([photo(0), photo(1), photo(2)]));
        ({ router } = show());
        await user.click(await tile(0));
      });

      it("should move between the photos shown with the arrow keys and buttons", async () => {
        expect(screen.getByRole("button", { name: "Previous photo" }).getAttribute("aria-disabled")).toBe("true");

        await user.keyboard("{ArrowRight}");
        expect(screen.getByRole("dialog", { name: `${tileName(1)}, 2 of 3` })).toBeTruthy();
        await user.click(screen.getByRole("button", { name: "Next photo" }));
        expect(screen.getByRole("dialog", { name: `${tileName(2)}, 3 of 3` })).toBeTruthy();
        // At the end Next stays where it is, focused, and does nothing.
        const next = screen.getByRole("button", { name: "Next photo" });
        expect(next.getAttribute("aria-disabled")).toBe("true");
        expect(document.activeElement).toBe(next);
        await user.click(next);
        await user.keyboard("{ArrowRight}");
        expect(screen.getByRole("dialog", { name: `${tileName(2)}, 3 of 3` })).toBeTruthy();
        await user.keyboard("{ArrowLeft}");
        expect(screen.getByRole("dialog", { name: `${tileName(1)}, 2 of 3` })).toBeTruthy();
        expect(router.state.location.search).toBe("?photo=p-1");
      });
    });

    describe("and the gallery holds one photo", () => {
      let router: Router;
      let opener: HTMLElement;

      beforeEach(async () => {
        net.fetchUserPhotos.mockResolvedValue(page([photo(0)]));
        ({ router } = show());
        opener = await tile(0);
        await user.click(opener);
      });

      it("should start on Close, keep Tab inside, and on Escape close and return focus to the photo", async () => {
        const dialog = screen.getByRole("dialog");
        expect(dialog).toBeInstanceOf(HTMLDialogElement);
        const close = within(dialog).getByRole("button", { name: "Close" });
        expect(document.activeElement).toBe(close);
        expect(document.documentElement.style.overflow).toBe("hidden");

        await user.keyboard("{Shift>}{Tab}{/Shift}");
        expect(dialog.contains(document.activeElement)).toBe(true);
        expect(document.activeElement).not.toBe(close);
        await user.tab();
        expect(document.activeElement).toBe(close);

        await user.keyboard("{Escape}");

        expect(screen.queryByRole("dialog")).toBeNull();
        expect(document.activeElement).toBe(opener);
        expect(router.state.location.search).toBe("");
        expect(document.documentElement.style.overflow).toBe("");
      });

      it("should show no Previous button", () => {
        expect(screen.queryByRole("button", { name: "Previous photo" })).toBeNull();
      });

      it("should show no Next button", () => {
        expect(screen.queryByRole("button", { name: "Next photo" })).toBeNull();
      });

      it("should close from its Close button and from the backdrop, not from a click on the photo", async () => {
        await user.click(screen.getByRole("img", { name: "Plaza 0" }));
        expect(screen.getByRole("dialog")).toBeTruthy();
        await user.click(screen.getByRole("button", { name: "Close" }));
        expect(screen.queryByRole("dialog")).toBeNull();

        await user.click(screen.getByRole("button", { name: tileName(0) }));
        const backdrop = screen.getByRole("dialog");
        fireEvent.pointerDown(backdrop, { clientX: -10, clientY: -10 });
        fireEvent.click(backdrop, { clientX: -10, clientY: -10 });
        expect(screen.queryByRole("dialog")).toBeNull();
      });

      it("should hide and show the information panel", async () => {
        const info = screen.getByRole("button", { name: "Information" });
        expect(info.getAttribute("aria-pressed")).toBe("true");

        await user.click(info);

        expect(info.getAttribute("aria-pressed")).toBe("false");
        expect(screen.queryByRole("heading", { name: "information" })).toBeNull();
        await user.click(info);
        expect(screen.getByRole("heading", { name: "information" })).toBeTruthy();
      });

      it("should keep Shift+Tab inside after a click on the photo", async () => {
        const dialog = screen.getByRole("dialog");

        await user.click(screen.getByRole("img", { name: "Plaza 0" }));
        expect(dialog.contains(document.activeElement)).toBe(true);
        await user.keyboard("{Shift>}{Tab}{/Shift}");

        expect(dialog.contains(document.activeElement)).toBe(true);
        expect(document.activeElement).not.toBe(dialog);
      });
    });

    describe("and the gallery holds two photos", () => {
      let router: Router;

      beforeEach(async () => {
        net.fetchUserPhotos.mockResolvedValue(page([photo(0), photo(1)]));
        ({ router } = show());
        await user.click(await tile(0));
      });

      it("should close on Back after stepping, without adding history", async () => {
        await user.keyboard("{ArrowRight}");
        expect(router.state.location.search).toBe("?photo=p-1");

        await act(async () => {
          await router.navigate(-1);
        });

        expect(router.state.location.search).toBe("");
        expect(screen.queryByRole("dialog")).toBeNull();
      });

      it("should keep arrow navigation working after clicking the media around a photo", async () => {
        await user.click(screen.getByRole("img", { name: "Plaza 0" }).parentElement!);
        await user.keyboard("{ArrowRight}");

        expect(screen.getByRole("dialog", { name: `${tileName(1)}, 2 of 2` })).toBeTruthy();
      });

      it("should keep its keys working after a click on the photo", async () => {
        await user.click(screen.getByRole("img", { name: "Plaza 0" }));
        expect(screen.getByRole("dialog").contains(document.activeElement)).toBe(true);
        await user.tab();
        expect(screen.getByRole("dialog").contains(document.activeElement)).toBe(true);
        await user.keyboard("{ArrowRight}");
        expect(screen.getByRole("dialog", { name: `${tileName(1)}, 2 of 2` })).toBeTruthy();
        await user.keyboard("{Escape}");

        expect(screen.queryByRole("dialog")).toBeNull();
      });
    });

    describe("and the next photo has no coordinates while its Jump In link is focused", () => {
      beforeEach(async () => {
        net.fetchUserPhotos.mockResolvedValue(page([photo(0), photo(1, { scene: { name: "Nowhere", x: "", y: "" } })]));
        show();
        await user.click(await tile(0));
        screen.getByRole("link", { name: "Jump in to Plaza 0 0,1" }).focus();
        await user.keyboard("{ArrowRight}");
      });

      it("should keep focus in the dialog when the step removes the focused link", async () => {
        const dialog = screen.getByRole("dialog", { name: `Photo at Nowhere, ${date}, 2 of 2` });
        expect(dialog.contains(document.activeElement)).toBe(true);
        await user.keyboard("{Escape}");
        expect(screen.queryByRole("dialog")).toBeNull();
      });
    });

    describe("and more photos exist than are loaded", () => {
      beforeEach(async () => {
        net.fetchUserPhotos.mockResolvedValue(page([photo(0)], 30));
        show();
        await user.click(await tile(0));
      });

      it("should count the position against the photos loaded, the ones Next can reach", () => {
        expect(screen.getByRole("dialog", { name: `${tileName(0)}, 1 of 1` })).toBeTruthy();
      });
    });

    describe("and the photo has an author and a date", () => {
      beforeEach(async () => {
        net.fetchUserPhotos.mockResolvedValue(page([photo(0, { author: { name: "kurd #1", address: A }, dateTime: "2026-11-10T23:30:00Z" })]));
        show();
        await user.click(await screen.findByRole("button", { name: /^Photo at Plaza 0/ }));
      });

      it("should name the download after who took it and the day", () => {
        expect(screen.getByRole("link", { name: "Download photo" }).getAttribute("download")).toBe("kurd_1-2026-11-10.jpg");
      });
    });

    describe.each([
      ["an author name with no usable character", { author: { name: "###", address: A } }, "photo-2026-11-10.jpg"],
      ["an unreadable date", { dateTime: "someday" }, "kurd.jpg"],
    ] as const)("and the photo has %s", (_case, over, fileName) => {
      beforeEach(async () => {
        net.fetchUserPhotos.mockResolvedValue(page([photo(0, { dateTime: "2026-11-10T23:30:00Z", ...over })]));
        show();
        await user.click(await screen.findByRole("button", { name: /^Photo at Plaza 0/ }));
      });

      it("should name the download without what it cannot read", () => {
        expect(screen.getByRole("link", { name: "Download photo" }).getAttribute("download")).toBe(fileName);
      });
    });

    describe("and the page runs under StrictMode's double effects", () => {
      let opener: HTMLElement;

      beforeEach(async () => {
        net.fetchUserPhotos.mockResolvedValue(page([photo(0)]));
        show({ strict: true });
        opener = await tile(0);
        await user.click(opener);
      });

      it("should return focus to the photo on close", async () => {
        expect(document.activeElement).toBe(screen.getByRole("button", { name: "Close" }));

        await user.keyboard("{Escape}");

        expect(screen.queryByRole("dialog")).toBeNull();
        expect(document.activeElement).toBe(opener);
      });
    });

    describe("and the information panel holds no link", () => {
      beforeEach(async () => {
        net.fetchUserPhotos.mockResolvedValue(
          page([photo(0, { author: { name: "", address: "not-a-wallet" }, scene: { name: "Nowhere", x: "", y: "" } })]),
        );
        show();
        await user.click(await screen.findByRole("button", { name: /^Photo at Nowhere/ }));
      });

      it("should let the keyboard reach the panel", async () => {
        const panel = screen.getByRole("region", { name: "information" });

        await user.keyboard("{Shift>}{Tab}{/Shift}");

        expect(document.activeElement).toBe(panel);
        // Without a parcel the place is the scene alone, not "Nowhere ,".
        expect(within(panel).getByText("Nowhere")).toBeTruthy();
        expect(within(panel).queryByText(/Nowhere ,/)).toBeNull();
      });
    });
  });

  describe("when a link names a photo", () => {
    describe("and the photo is in the loaded pages", () => {
      beforeEach(() => {
        net.fetchUserPhotos.mockResolvedValue(page([photo(0), photo(1)]));
        show({ path: `/profile/${A}/photos?photo=p-1` });
      });

      it("should open it", async () => {
        expect(await screen.findByRole("dialog", { name: `${tileName(1)}, 2 of 2` })).toBeTruthy();
      });
    });

    describe("and the service does not have the photo", () => {
      let router: Router;

      beforeEach(async () => {
        net.fetchPhoto.mockResolvedValue(null);
        net.fetchUserPhotos.mockResolvedValue(page([photo(0), photo(1)]));
        ({ router } = show({ path: `/profile/${A}/photos?photo=${uuid(9)}&tab=x` }));
        await tile(0);
      });

      it("should drop the link, keeping the rest of the query", async () => {
        await waitFor(() => expect(router.state.location.search).toBe("?tab=x"));
        expect(screen.queryByRole("dialog")).toBeNull();
        expect(net.fetchPhoto.mock.calls[0]?.[0]).toBe(uuid(9));
      });
    });

    describe.each(["..", "", "not-a-photo"])("and the id is %j", (id) => {
      let router: Router;

      beforeEach(() => {
        net.fetchUserPhotos.mockResolvedValue(page([photo(0)], 40));
        ({ router } = show({ path: `/profile/${A}/photos?photo=${encodeURIComponent(id)}` }));
      });

      it("should drop the link without reading anything", async () => {
        await waitFor(() => expect(router.state.location.search).toBe(""));
        expect(net.fetchPhoto).not.toHaveBeenCalled();
      });
    });

    describe("and the photo is past the pages loaded", () => {
      beforeEach(() => {
        net.fetchUserPhotos.mockResolvedValue(page([photo(0)], 40));
        net.fetchPhoto.mockResolvedValue(photo(33, { id: uuid(33) }));
        show({ path: `/profile/${A}/photos?photo=${uuid(33)}` });
      });

      it("should read it by id and show it alone", async () => {
        const dialog = await screen.findByRole("dialog", { name: `${tileName(33)}, 1 of 1` });

        expect(within(dialog).getByRole("img", { name: "Plaza 33" })).toBeTruthy();
        expect(screen.queryByRole("button", { name: "Next photo" })).toBeNull();
      });
    });

    describe.each([
      ["another member's photo", () => photo(5, { id: uuid(5), author: { name: "x", address: FRIEND } })],
      ["a private photo, to a visitor", () => photo(5, { id: uuid(5), isPublic: false })],
    ])("and the photo is %s", (_case, linked) => {
      let router: Router;

      beforeEach(() => {
        net.fetchUserPhotos.mockResolvedValue(page([photo(0)], 40));
        net.fetchPhoto.mockResolvedValue(linked());
        ({ router } = show({ path: `/profile/${A}/photos?photo=${uuid(5)}` }));
      });

      it("should drop the link", async () => {
        await waitFor(() => expect(router.state.location.search).toBe(""));
        expect(screen.queryByRole("dialog")).toBeNull();
      });
    });

    describe("and the photo cannot be read", () => {
      let router: Router;

      beforeEach(async () => {
        net.fetchUserPhotos.mockResolvedValue(page([photo(0)], 40));
        net.fetchPhoto.mockRejectedValue(new Error("503"));
        ({ router } = show({ path: `/profile/${A}/photos?photo=${uuid(5)}` }));
        await tile(0);

        await waitFor(() => expect(net.fetchPhoto).toHaveBeenCalled());
        await act(async () => new Promise((resolve) => setTimeout(resolve, 0)));
      });

      it("should keep the link, opening nothing", () => {
        expect(router.state.location.search).toBe(`?photo=${uuid(5)}`);
        expect(screen.queryByRole("dialog")).toBeNull();
      });
    });
  });
});
