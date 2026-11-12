// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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
const identity = { signer: A, ephemeral: { address: "0xeph", privateKey: "0x01" }, expiration: "2999-01-01T00:00:00.000Z", authChain: [] };
const tabs = [
  { id: "overview", label: "Overview", href: `/profile/${A}` },
  { id: "photos", label: "Photos", href: `/profile/${A}/photos` },
];
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

function show({ own = false, path = `/profile/${A}/photos` } = {}) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter(
    [{ path: "*", element: <ProfilePhotosScreen profile={profile()} own={own} tabs={tabs} onTab={vi.fn()} /> }],
    { initialEntries: [path] },
  );
  render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return { client, router };
}

const tile = (n: number) => screen.findByRole("button", { name: tileName(n) });
/** An id in the service's UUID format, for photos a link reads by id. */
const uuid = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, "0")}`;

beforeEach(() => {
  auth.state = { identity: null, isConnected: false, address: null };
  net.fetchCommunityProfiles.mockResolvedValue({});
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  document.body.style.overflow = "";
});

describe("profile photos, the grid", () => {
  it("shows a member's public photos, a page at a time, read unsigned", async () => {
    net.fetchUserPhotos.mockResolvedValue(page([photo(0), photo(1)], 30));

    show();

    const first = await tile(0);
    expect(first.querySelector("img")?.getAttribute("src")).toBe("https://reel.example/p-0-thumbnail.jpg");
    expect(await tile(1)).toBeTruthy();
    expect(screen.getByText("Showing 2 of 30 photos")).toBeTruthy();
    expect(net.fetchUserPhotos).toHaveBeenCalledWith(A, { limit: PROFILE_COLLECTIBLES_PAGE, offset: 0 }, { signal: expect.any(AbortSignal) });
    expect(screen.queryByRole("group", { name: "Photos shown" })).toBeNull();
  });

  it("tells a visitor the member has taken no photos", async () => {
    net.fetchUserPhotos.mockResolvedValue(page([]));

    show();

    expect(await screen.findByText("This member has not taken any photos yet.")).toBeTruthy();
    expect(screen.queryByText("No photos yet")).toBeNull();
  });

  it("invites the owner to take their first photo in-world", async () => {
    auth.state = { identity, isConnected: true, address: A };
    net.fetchUserPhotos.mockResolvedValue(page([]));

    show({ own: true });

    const title = await screen.findByText("No photos yet", { selector: "p" });
    expect(within(title.parentElement!).getByRole("link", { name: /Jump in/ }).getAttribute("href")).toBe(playUrl());
  });

  it("says the read failed, never an empty gallery, and tries again", async () => {
    const user = userEvent.setup();
    net.fetchUserPhotos.mockRejectedValueOnce(new Error("503")).mockResolvedValueOnce(page([photo(0)]));
    show();

    expect(await screen.findByText("Photos are unavailable right now.")).toBeTruthy();
    expect(screen.queryByText("This member has not taken any photos yet.")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Try again" }));

    expect(await tile(0)).toBeTruthy();
  });

  it("keeps the photos on screen when a background re-read fails", async () => {
    net.fetchUserPhotos.mockResolvedValueOnce(page([photo(0)])).mockRejectedValueOnce(new Error("503"));
    const { client } = show();
    await tile(0);

    await act(async () => {
      await client.refetchQueries({ queryKey: profileKeys.photos(A) });
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    expect(client.getQueryState(profileKeys.photos(A))?.status).toBe("error");
    expect(screen.getByRole("button", { name: tileName(0) })).toBeTruthy();
    expect(screen.queryByText("Photos are unavailable right now.")).toBeNull();
  });

  it("loads more after the photos shown and keeps them when the next page fails", async () => {
    const user = userEvent.setup();
    const first = Array.from({ length: PROFILE_COLLECTIBLES_PAGE }, (_, n) => photo(n));
    net.fetchUserPhotos
      .mockResolvedValueOnce(page(first, 30))
      .mockRejectedValueOnce(new Error("503"))
      .mockResolvedValueOnce(page([photo(24)], 30));
    show();
    await tile(0);

    await user.click(screen.getByRole("button", { name: "Load more" }));

    expect(await screen.findByText("More photos could not be loaded.")).toBeTruthy();
    expect(screen.getByRole("button", { name: tileName(23) })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await tile(24)).toBeTruthy();
    expect(net.fetchUserPhotos.mock.calls[2]?.[1]).toEqual({ limit: PROFILE_COLLECTIBLES_PAGE, offset: PROFILE_COLLECTIBLES_PAGE });
    expect(screen.getByText("Showing 25 of 30 photos")).toBeTruthy();
  });
});

describe("profile photos, the owner's visibility filter", () => {
  beforeEach(() => {
    auth.state = { identity, isConnected: true, address: A };
  });

  it("reads the owner's gallery signed and opens on the public photos", async () => {
    net.fetchUserPhotos.mockResolvedValue(page([photo(0), photo(1, { isPublic: false })], 2));

    const { client } = show({ own: true });

    expect(await tile(0)).toBeTruthy();
    expect(screen.queryByRole("button", { name: `${tileName(1)}, private` })).toBeNull();
    expect(net.fetchUserPhotos.mock.calls[0]?.[2]).toMatchObject({ identity });
    expect(client.getQueryData(profileKeys.myPhotos(A))).toBeDefined();
    expect(client.getQueryData(profileKeys.photos(A))).toBeUndefined();
    expect(screen.getByRole("button", { name: "Public" }).getAttribute("aria-pressed")).toBe("true");
  });

  it("shows the private photos, marked, and every photo under All", async () => {
    const user = userEvent.setup();
    net.fetchUserPhotos.mockResolvedValue(page([photo(0), photo(1, { isPublic: false })], 2));
    show({ own: true });
    await tile(0);

    await user.click(screen.getByRole("button", { name: "Private" }));
    expect(screen.getByRole("button", { name: `${tileName(1)}, private` })).toBeTruthy();
    expect(screen.queryByRole("button", { name: tileName(0) })).toBeNull();
    expect(screen.getByText("1 photo")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "All" }));
    expect(screen.getByRole("button", { name: tileName(0) })).toBeTruthy();
    expect(screen.getByRole("button", { name: `${tileName(1)}, private` })).toBeTruthy();
    expect(screen.getByText("2 photos")).toBeTruthy();
  });

  it("says when no loaded photo matches, still offering the next page", async () => {
    const user = userEvent.setup();
    net.fetchUserPhotos.mockResolvedValue(page([photo(0)], 40));
    show({ own: true });
    await tile(0);

    await user.click(screen.getByRole("button", { name: "Private" }));

    expect(screen.getByText("No photos match this filter.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Load more" })).toBeTruthy();
  });

  it("waits for the signed list instead of saying the owner has no photos", async () => {
    // The page first reads the public list (no identity yet), then the owner's identity arrives.
    auth.state = { identity: null, isConnected: true, address: A };
    let release!: (value: unknown) => void;
    net.fetchUserPhotos.mockImplementation((_a: string, _p: unknown, opts: { identity?: unknown }) =>
      opts.identity ? new Promise((resolve) => (release = resolve)) : Promise.resolve(page([])),
    );
    show({ own: true });
    expect(await screen.findByText("No photos yet", { selector: "p" })).toBeTruthy();

    act(() => auth.set({ identity, isConnected: true, address: A }));

    expect(screen.queryByText("No photos yet", { selector: "p" })).toBeNull();
    expect(screen.getByText("Loading photos")).toBeTruthy();
    await act(async () => release(page([photo(1, { isPublic: false })])));
    await userEvent.setup().click(await screen.findByRole("button", { name: "Private" }));
    expect(screen.getByRole("button", { name: `${tileName(1)}, private` })).toBeTruthy();
  });

  it("keeps the public grid when the signed read fails, without the filter, and tries again", async () => {
    const user = userEvent.setup();
    auth.state = { identity: null, isConnected: true, address: A };
    net.fetchUserPhotos.mockImplementation(async (_a: string, _p: unknown, opts: { identity?: unknown }) => {
      if (opts.identity) throw new Error("401");
      return page([photo(0)]);
    });
    show({ own: true });
    await tile(0);

    act(() => auth.set({ identity, isConnected: true, address: A }));

    expect(await screen.findByText("Your private photos could not be loaded.", { exact: false })).toBeTruthy();
    expect(screen.getByRole("button", { name: tileName(0) })).toBeTruthy();
    expect(screen.queryByRole("group", { name: "Photos shown" })).toBeNull();
    expect(screen.queryByText("Photos are unavailable right now.")).toBeNull();
    net.fetchUserPhotos.mockResolvedValue(page([photo(0), photo(1, { isPublic: false })]));
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("group", { name: "Photos shown" })).toBeTruthy();
  });

  it("reads the public gallery, without the filter, when the owner has no identity in this browser", async () => {
    auth.state = { identity: null, isConnected: true, address: A };
    net.fetchUserPhotos.mockResolvedValue(page([photo(0)]));

    show({ own: true });

    expect(await tile(0)).toBeTruthy();
    expect(net.fetchUserPhotos.mock.calls[0]?.[2]).not.toHaveProperty("identity");
    expect(screen.queryByRole("group", { name: "Photos shown" })).toBeNull();
  });

  it("never signs a visitor's read with their own identity", async () => {
    auth.state = { identity: { ...identity, signer: FRIEND }, isConnected: true, address: FRIEND };
    net.fetchUserPhotos.mockResolvedValue(page([photo(0)]));

    show({ own: false });

    expect(await tile(0)).toBeTruthy();
    expect(net.fetchUserPhotos.mock.calls[0]?.[2]).not.toHaveProperty("identity");
  });
});

describe("profile photos, the viewer", () => {
  const people = [
    { name: "Alice", address: FRIEND, isGuest: false },
    { name: "", address: "0x2222222222222222222222222222222222222222", isGuest: true },
    { name: "Ghost", address: "not-a-wallet", isGuest: true },
  ];

  it("opens a photo with its date, author, place and the people in it", async () => {
    const user = userEvent.setup();
    net.fetchCommunityProfiles.mockResolvedValue({ [FRIEND]: { name: "Alice", picture: "https://faces.example/alice.png" } });
    net.fetchUserPhotos.mockResolvedValue(page([photo(0, { people }), photo(1)]));
    const { router } = show();

    await user.click(await tile(0));

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

  it("jumps in to the world a photo was taken in, never to an uploaded realm URL, and offers no Jump In without coordinates", async () => {
    const user = userEvent.setup();
    net.fetchUserPhotos.mockResolvedValue(
      page([
        photo(0, { realm: "sample.dcl.eth", scene: { name: "World", x: "3", y: "-4" } }),
        photo(1, { scene: { name: "Nowhere", x: "", y: "" } }),
        photo(2, { realm: "https://peer.example", scene: { name: "Server", x: "5", y: "5" } }),
      ]),
    );
    show();

    await user.click(await screen.findByRole("button", { name: `Photo at World, ${date}` }));
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

  it("moves between the photos shown with the arrow keys and buttons", async () => {
    const user = userEvent.setup();
    net.fetchUserPhotos.mockResolvedValue(page([photo(0), photo(1), photo(2)]));
    const { router } = show();
    await user.click(await tile(0));
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

  it("starts on Close, keeps Tab inside, and on Escape closes and returns focus to the photo", async () => {
    const user = userEvent.setup();
    net.fetchUserPhotos.mockResolvedValue(page([photo(0)]));
    const { router } = show();
    const opener = await tile(0);
    await user.click(opener);
    const dialog = screen.getByRole("dialog");
    const close = within(dialog).getByRole("button", { name: "Close" });
    expect(document.activeElement).toBe(close);
    expect(document.body.style.overflow).toBe("hidden");

    await user.keyboard("{Shift>}{Tab}{/Shift}");
    expect(dialog.contains(document.activeElement)).toBe(true);
    expect(document.activeElement).not.toBe(close);
    await user.tab();
    expect(document.activeElement).toBe(close);

    await user.keyboard("{Escape}");

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(opener);
    expect(router.state.location.search).toBe("");
    expect(document.body.style.overflow).toBe("");
  });

  it("shows no Previous or Next for a single photo", async () => {
    const user = userEvent.setup();
    net.fetchUserPhotos.mockResolvedValue(page([photo(0)]));
    show();

    await user.click(await tile(0));

    expect(screen.queryByRole("button", { name: "Previous photo" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Next photo" })).toBeNull();
  });

  it("closes from its Close button and from the backdrop, not from a click on the photo", async () => {
    const user = userEvent.setup();
    net.fetchUserPhotos.mockResolvedValue(page([photo(0)]));
    show();
    await user.click(await tile(0));

    await user.click(screen.getByRole("img", { name: "Plaza 0" }));
    expect(screen.getByRole("dialog")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(screen.queryByRole("dialog")).toBeNull();

    await user.click(screen.getByRole("button", { name: tileName(0) }));
    await user.click(screen.getByRole("dialog").parentElement!);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("hides and shows the information panel", async () => {
    const user = userEvent.setup();
    net.fetchUserPhotos.mockResolvedValue(page([photo(0)]));
    show();
    await user.click(await tile(0));
    const info = screen.getByRole("button", { name: "Information" });
    expect(info.getAttribute("aria-pressed")).toBe("true");

    await user.click(info);

    expect(info.getAttribute("aria-pressed")).toBe("false");
    expect(screen.queryByRole("heading", { name: "information" })).toBeNull();
    await user.click(info);
    expect(screen.getByRole("heading", { name: "information" })).toBeTruthy();
  });

  it("opens the photo a link names, and drops a link to one the service does not have", async () => {
    net.fetchPhoto.mockResolvedValue(null);
    net.fetchUserPhotos.mockResolvedValue(page([photo(0), photo(1)]));
    show({ path: `/profile/${A}/photos?photo=p-1` });

    expect(await screen.findByRole("dialog", { name: `${tileName(1)}, 2 of 2` })).toBeTruthy();
    cleanup();

    const { router } = show({ path: `/profile/${A}/photos?photo=${uuid(9)}&tab=x` });
    await tile(0);

    await waitFor(() => expect(router.state.location.search).toBe("?tab=x"));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(net.fetchPhoto.mock.calls[0]?.[0]).toBe(uuid(9));
  });

  it.each(["..", "", "not-a-photo"])("drops a link to %j without reading anything", async (id) => {
    net.fetchUserPhotos.mockResolvedValue(page([photo(0)], 40));
    const { router } = show({ path: `/profile/${A}/photos?photo=${encodeURIComponent(id)}` });

    await waitFor(() => expect(router.state.location.search).toBe(""));
    expect(net.fetchPhoto).not.toHaveBeenCalled();
  });

  it("reads a linked photo past the pages loaded by id, and shows it alone", async () => {
    net.fetchUserPhotos.mockResolvedValue(page([photo(0)], 40));
    net.fetchPhoto.mockResolvedValue(photo(33, { id: uuid(33) }));
    show({ path: `/profile/${A}/photos?photo=${uuid(33)}` });

    const dialog = await screen.findByRole("dialog", { name: `${tileName(33)}, 1 of 1` });

    expect(within(dialog).getByRole("img", { name: "Plaza 33" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Next photo" })).toBeNull();
  });

  it.each([
    ["another member's photo", photo(5, { id: uuid(5), author: { name: "x", address: FRIEND } })],
    ["a private photo, to a visitor", photo(5, { id: uuid(5), isPublic: false })],
  ])("drops a link to %s", async (_case, linked) => {
    net.fetchUserPhotos.mockResolvedValue(page([photo(0)], 40));
    net.fetchPhoto.mockResolvedValue(linked);
    const { router } = show({ path: `/profile/${A}/photos?photo=${uuid(5)}` });

    await waitFor(() => expect(router.state.location.search).toBe(""));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("keeps the link, opening nothing, while the photo cannot be read", async () => {
    net.fetchUserPhotos.mockResolvedValue(page([photo(0)], 40));
    net.fetchPhoto.mockRejectedValue(new Error("503"));
    const { router } = show({ path: `/profile/${A}/photos?photo=${uuid(5)}` });
    await tile(0);

    await waitFor(() => expect(net.fetchPhoto).toHaveBeenCalled());
    await act(async () => new Promise((resolve) => setTimeout(resolve, 0)));

    expect(router.state.location.search).toBe(`?photo=${uuid(5)}`);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("opens the owner's link to their own private photo past the pages loaded", async () => {
    auth.state = { identity, isConnected: true, address: A };
    net.fetchUserPhotos.mockResolvedValue(page([photo(0)], 40));
    net.fetchPhoto.mockResolvedValue(photo(30, { id: uuid(30), isPublic: false }));

    show({ own: true, path: `/profile/${A}/photos?photo=${uuid(30)}` });

    expect(await screen.findByRole("dialog", { name: `${tileName(30)}, 1 of 1` })).toBeTruthy();
  });

  it("opens the owner's link to their own private photo, showing every photo", async () => {
    auth.state = { identity, isConnected: true, address: A };
    net.fetchUserPhotos.mockResolvedValue(page([photo(0), photo(1, { isPublic: false })]));
    show({ own: true, path: `/profile/${A}/photos?photo=p-1` });

    expect(await screen.findByRole("dialog", { name: `${tileName(1)}, 2 of 2` })).toBeTruthy();
    expect(screen.getByRole("button", { name: "All" }).getAttribute("aria-pressed")).toBe("true");
    expect(net.fetchPhoto).not.toHaveBeenCalled();
  });

  it("counts the position against the photos loaded, the ones Next can reach", async () => {
    const user = userEvent.setup();
    net.fetchUserPhotos.mockResolvedValue(page([photo(0)], 30));
    show();

    await user.click(await tile(0));

    expect(screen.getByRole("dialog", { name: `${tileName(0)}, 1 of 1` })).toBeTruthy();
  });

  it("closes on Back after opening a photo from the grid, and steps without adding history", async () => {
    const user = userEvent.setup();
    net.fetchUserPhotos.mockResolvedValue(page([photo(0), photo(1)]));
    const { router } = show();
    await user.click(await tile(0));
    await user.keyboard("{ArrowRight}");
    expect(router.state.location.search).toBe("?photo=p-1");

    await act(async () => {
      await router.navigate(-1);
    });

    expect(router.state.location.search).toBe("");
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("names the download after who took it and the day", async () => {
    const user = userEvent.setup();
    net.fetchUserPhotos.mockResolvedValue(page([photo(0, { author: { name: "kurd #1", address: A }, dateTime: "2026-11-10T23:30:00Z" })]));
    show();

    await user.click(await screen.findByRole("button", { name: /^Photo at Plaza 0/ }));

    expect(screen.getByRole("link", { name: "Download photo" }).getAttribute("download")).toBe("kurd_1-2026-11-10.jpg");
  });

  it("keeps Shift+Tab inside after a click on the photo", async () => {
    const user = userEvent.setup();
    net.fetchUserPhotos.mockResolvedValue(page([photo(0)]));
    show();
    await user.click(await tile(0));
    const dialog = screen.getByRole("dialog");

    await user.click(screen.getByRole("img", { name: "Plaza 0" }));
    expect(document.activeElement).toBe(dialog);
    await user.keyboard("{Shift>}{Tab}{/Shift}");

    expect(dialog.contains(document.activeElement)).toBe(true);
    expect(document.activeElement).not.toBe(dialog);
  });

  it("keeps focus in the dialog when a step removes the focused link", async () => {
    const user = userEvent.setup();
    net.fetchUserPhotos.mockResolvedValue(page([photo(0), photo(1, { scene: { name: "Nowhere", x: "", y: "" } })]));
    show();
    await user.click(await tile(0));
    const jumpIn = screen.getByRole("link", { name: "Jump in to Plaza 0 0,1" });
    jumpIn.focus();

    await user.keyboard("{ArrowRight}");

    const dialog = screen.getByRole("dialog", { name: `Photo at Nowhere, ${date}, 2 of 2` });
    expect(dialog.contains(document.activeElement)).toBe(true);
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("keeps its keys working after a click on the photo", async () => {
    const user = userEvent.setup();
    net.fetchUserPhotos.mockResolvedValue(page([photo(0), photo(1)]));
    show();
    await user.click(await tile(0));

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
