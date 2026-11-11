// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ComponentProps, ReactNode } from "react";
import { createRoutesStub, useLocation } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { destination } from "@/data/lib/catalyst/places/destination.fixture";
import { fetchDestinations, type Destination, type DestinationParams } from "@/data/lib/catalyst/places/destinations";
import { openSignIn } from "@/features/components/auth/signin-store";

import Places from "./places";
import { track } from "@/core/lib/telemetry/track";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

const auth = vi.hoisted(() => ({ current: { isConnected: false, identity: null as null | { signer: string } } }));
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => auth.current }));
vi.mock("@/features/components/auth/signin-store", () => ({ openSignIn: vi.fn() }));
vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));
vi.mock("@ui/web/frames/SitesChrome", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));
vi.mock("@/data/lib/catalyst/places/destinations", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/places/destinations")>()),
  fetchDestinations: vi.fn(),
}));

const DESKTOP = { kind: "desktop", os: "macos", arch: "unknown" } as const;
const MEMBER = "0x00000000000000000000000000000000000000cc";

type Feeds = { browse?: Destination[] | Error; live?: Destination[]; member?: Destination[] | Error; total?: number };

function serve(feeds: Feeds) {
  vi.mocked(fetchDestinations).mockImplementation(async (params: DestinationParams = {}) => {
    const pick = params.only_favorites || params.owner ? feeds.member : params.limit === 40 ? feeds.live : feeds.browse;
    if (pick instanceof Error) throw pick;
    const data = pick ?? [];
    return { data, total: feeds.total ?? data.length, received: data.length, copy: null };
  });
}

function Location() {
  const location = useLocation();
  return <output data-testid="location">{location.pathname + location.search}</output>;
}

function renderPage(url = "/places", platform: object = DESKTOP, unavailable = false, loaderSid: string | null = "sid") {
  const Stub = createRoutesStub([
    {
      id: "root",
      path: "/",
      loader: () => ({ sid: "root-session" }),
      children: [
        {
          path: "places",
          Component: (props: ComponentProps<typeof Places>) => (
            <>
              <Places {...props} />
              <Location />
            </>
          ),
          loader: () => ({ sid: loaderSid ?? undefined, platform, dehydratedState: undefined, unavailable }),
        },
      ],
    },
    { path: "/places/:id", Component: () => <p>Place page</p> },
  ]);
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <Stub initialEntries={[url]} />
    </QueryClientProvider>,
  );
  return { user: userEvent.setup() };
}

const location = () => screen.getByTestId("location").textContent ?? "";

beforeEach(() => {
  auth.current = { isConnected: false, identity: null };
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => Response.json([])),
  );
  Element.prototype.scrollIntoView = vi.fn();
});

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  vi.unstubAllGlobals();
});

describe("when the Explore grid loads", () => {
  it("should list the feed and leave out what Live Now already shows", async () => {
    const busy = destination({ id: "busy", title: "Busy", user_count: 5 });
    serve({ browse: [busy, destination({ id: "quiet", title: "Quiet" })], live: [busy] });
    renderPage();
    await screen.findByRole("link", { name: "Quiet" });
    const grid = screen.getByRole("list", { name: "Explore all" });
    expect(within(grid).getByRole("link", { name: "Quiet" })).toBeTruthy();
    expect(within(grid).queryByRole("link", { name: "Busy" })).toBeNull();
    expect(screen.getByRole("heading", { name: "Live Now" })).toBeTruthy();
  });

  it("should retain the existing root session for client-navigation list and click analytics", async () => {
    serve({ browse: [destination({ title: "Quiet" })] });
    const { user } = renderPage("/places", DESKTOP, false, null);
    await user.click(await screen.findByRole("link", { name: "Quiet" }));
    expect(track).toHaveBeenCalledWith("place_list_viewed", expect.anything(), { sid: "root-session", story: "misc/browse-places" });
    expect(track).toHaveBeenCalledWith("place_card_clicked", expect.anything(), { sid: "root-session", story: "misc/browse-places" });
  });

  it("should say so when nothing matches", async () => {
    serve({ browse: [] });
    renderPage("/places?category=music");
    expect(await screen.findByText("No places match your filters.")).toBeTruthy();
  });

  it("should show the error with Retry and recover when the feed answers again", async () => {
    serve({ browse: new Error("Catalyst returned 502") });
    const { user } = renderPage();
    expect((await screen.findByRole("alert")).textContent).toBe("Something went wrong loading places.");
    serve({ browse: [destination({ title: "Back" })] });
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByRole("link", { name: "Back" })).toBeTruthy();
  });

  it("should show the error of a 503 document without reading again until Retry", async () => {
    serve({ browse: [destination({ title: "Back" })] });
    const { user } = renderPage("/places", DESKTOP, true);
    expect((await screen.findByRole("alert")).textContent).toBe("Something went wrong loading places.");
    expect(fetchDestinations).not.toHaveBeenCalledWith(expect.objectContaining({ limit: 48 }), expect.anything());
    expect(screen.queryByText("Loading places…")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByRole("link", { name: "Back" })).toBeTruthy();
  });

  it("should read normally once the view changes after a 503 document", async () => {
    serve({ browse: [destination({ title: "Music place" })] });
    const { user } = renderPage("/places", DESKTOP, true);
    await screen.findByRole("alert");
    const [select] = screen.getAllByRole("combobox", { name: "Category" });
    await user.selectOptions(select!, "music");
    expect(await screen.findByRole("link", { name: "Music place" })).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
    await user.selectOptions(select!, "all");
    await waitFor(() => expect(location()).toBe("/places"));
    expect(await screen.findByRole("link", { name: "Music place" })).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("should offer the next page without claiming the catalogue is empty when a whole page is hidden", async () => {
    serve({ browse: Array.from({ length: 48 }, (_, i) => destination({ id: `e${i}`, title: "Empty" })), total: 100 });
    renderPage();
    expect(await screen.findByRole("button", { name: "Load more places" })).toBeTruthy();
    expect(screen.queryByText("No places match your filters.")).toBeNull();
  });

  it("should list a row repeated on the next page once", async () => {
    serve({ browse: Array.from({ length: 48 }, (_, i) => destination({ id: `p${i}`, title: `Place ${i}` })), total: 100 });
    const { user } = renderPage();
    const more = await screen.findByRole("button", { name: "Load more places" });
    serve({ browse: [destination({ id: "p47", title: "Place 47" }), destination({ id: "new", title: "New" })], total: 100 });
    await user.click(more);
    await screen.findByRole("link", { name: "New" });
    expect(screen.getAllByRole("link", { name: "Place 47" })).toHaveLength(1);
  });

  it("should read the faces of a new page's creators only", async () => {
    const owner = (n: number) => `0x${n.toString(16).padStart(40, "0")}`;
    serve({
      browse: Array.from({ length: 48 }, (_, i) => destination({ id: `p${i}`, title: `Place ${i}`, owner: owner(i + 1) })),
      total: 100,
    });
    const { user } = renderPage();
    const more = await screen.findByRole("button", { name: "Load more places" });
    await waitFor(() => expect(fetch).toHaveBeenCalledOnce());
    serve({ browse: [destination({ id: "p0", owner: owner(1) }), destination({ id: "new", title: "New", owner: owner(99) })], total: 100 });
    await user.click(more);
    await screen.findByRole("link", { name: "New" });
    await waitFor(() => expect(fetch).toHaveBeenCalledTimes(2));
    const body = JSON.parse(String((vi.mocked(fetch).mock.calls[1] as unknown as [string, RequestInit])[1].body)) as { ids: string[] };
    expect(body.ids).toEqual([owner(99)]);
  });

  it("should load the next page from the offset after the first", async () => {
    serve({ browse: Array.from({ length: 48 }, (_, i) => destination({ id: `p${i}`, title: `Place ${i}` })), total: 100 });
    const { user } = renderPage();
    await user.click(await screen.findByRole("button", { name: "Load more places" }));
    await waitFor(() => expect(fetchDestinations).toHaveBeenCalledWith(expect.objectContaining({ offset: 48 }), expect.anything()));
  });

  it("should keep the loaded cards and offer Retry when the next page fails", async () => {
    serve({ browse: Array.from({ length: 48 }, (_, i) => destination({ id: `p${i}`, title: `Place ${i}` })), total: 100 });
    const { user } = renderPage();
    const more = await screen.findByRole("button", { name: "Load more places" });
    serve({ browse: new Error("timeout") });
    await user.click(more);
    expect((await screen.findByRole("alert")).textContent).toBe("Something went wrong loading places.");
    expect(screen.getByRole("link", { name: "Place 0" })).toBeTruthy();
    serve({ browse: [destination({ id: "next", title: "Next page" })], total: 100 });
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByRole("link", { name: "Next page" })).toBeTruthy();
  });
});

describe("when the view changes", () => {
  it("should put the section in ?tab and drop it for Explore all", async () => {
    serve({ browse: [destination()] });
    const { user } = renderPage();
    await user.click(await screen.findByRole("button", { name: "Favourites" }));
    expect(location()).toBe("/places?tab=favourites");
    expect(screen.getByRole("button", { name: "Favourites" }).getAttribute("aria-pressed")).toBe("true");
    await user.click(screen.getByRole("button", { name: "Explore all" }));
    expect(location()).toBe("/places");
  });

  it("should put the category in ?category, filter the feed and clear it from the chip", async () => {
    serve({ browse: [destination()] });
    const { user } = renderPage();
    const [select] = await screen.findAllByRole("combobox", { name: "Category" });
    await user.selectOptions(select!, "music");
    expect(location()).toBe("/places?category=music");
    await waitFor(() =>
      expect(fetchDestinations).toHaveBeenCalledWith(expect.objectContaining({ categories: "music" }), expect.anything()),
    );
    await user.click(screen.getByRole("button", { name: "Remove filter: Music" }));
    expect(location()).toBe("/places");
  });

  it("should put the search in ?search once typing pauses and collapse Live Now", async () => {
    serve({ browse: [destination()], live: [destination({ id: "live", user_count: 3 })] });
    const { user } = renderPage();
    expect(await screen.findByRole("heading", { name: "Live Now" })).toBeTruthy();
    await user.type(screen.getByRole("searchbox", { name: "Search places" }), "plaza");
    await waitFor(() => expect(location()).toBe("/places?search=plaza"));
    await waitFor(() => expect(fetchDestinations).toHaveBeenCalledWith(expect.objectContaining({ search: "plaza" }), expect.anything()));
    expect(screen.queryByRole("heading", { name: "Live Now" })).toBeNull();
  });

  it("should not send a search shorter than three characters", async () => {
    serve({ browse: [destination()] });
    renderPage("/places?search=ab");
    await screen.findByRole("list", { name: "Explore all" });
    expect(screen.getByRole("searchbox", { name: "Search places" })).toHaveProperty("value", "ab");
    expect(fetchDestinations).not.toHaveBeenCalledWith(expect.objectContaining({ search: "ab" }), expect.anything());
  });
});

describe("when a member section is open", () => {
  it("should ask a signed-out visitor to sign in and read nothing for them", async () => {
    serve({ member: [destination()] });
    const { user } = renderPage("/places?tab=favourites");
    await user.click(await screen.findByRole("button", { name: "Sign In" }));
    expect(screen.getByText("Sign in and favourite places to find them here.")).toBeTruthy();
    expect(openSignIn).toHaveBeenCalledOnce();
    expect(fetchDestinations).not.toHaveBeenCalledWith(expect.objectContaining({ only_favorites: true }), expect.anything());
  });

  it("should read the member's favourites signed as them", async () => {
    auth.current = { isConnected: true, identity: { signer: MEMBER } };
    serve({ member: [destination({ title: "Saved" })] });
    renderPage("/places?tab=favourites");
    expect(await screen.findByRole("link", { name: "Saved" })).toBeTruthy();
    expect(fetchDestinations).toHaveBeenCalledWith(
      expect.objectContaining({ only_favorites: true }),
      expect.objectContaining({ identity: auth.current.identity }),
    );
  });

  it.each([
    ["favourites", null],
    ["my", null],
    ["my", "https://example.com/v2/map.png"],
    ["favourites", "https://example.com/bafkreidj26s7aenyxfthfdibnqonzqm5ptc4iamml744gmcyuokewkr76y"],
  ])("should preserve member %s rows with cover %s", async (tab, image) => {
    auth.current = { isConnected: true, identity: { signer: MEMBER } };
    serve({ member: [destination({ title: "Empty", image })] });
    renderPage(`/places?tab=${tab}`);
    expect(await screen.findByRole("link", { name: "Empty" })).toBeTruthy();
  });

  it("should list the member's own places by their address", async () => {
    auth.current = { isConnected: true, identity: { signer: MEMBER } };
    serve({ member: [] });
    renderPage("/places?tab=my");
    expect(await screen.findByText("You haven't published any places yet.")).toBeTruthy();
    expect(fetchDestinations).toHaveBeenCalledWith(expect.objectContaining({ owner: MEMBER }), expect.anything());
  });

  it("should open JUMP IN for a favourite on desktop even when people are in it", async () => {
    auth.current = { isConnected: true, identity: { signer: MEMBER } };
    serve({ member: [destination({ title: "Busy favourite", user_count: 7 })] });
    const { user } = renderPage("/places?tab=favourites");
    await user.click(await screen.findByRole("link", { name: "Busy favourite" }));
    expect(screen.getByRole("dialog", { name: "Busy favourite" })).toBeTruthy();
    expect(screen.getByText("7 people in-world")).toBeTruthy();
  });

  it("should read the new account's favourites after an account switch", async () => {
    auth.current = { isConnected: true, identity: { signer: MEMBER } };
    serve({ member: [destination({ title: "Saved" })] });
    const { user } = renderPage("/places?tab=favourites");
    await screen.findByRole("link", { name: "Saved" });
    const other = { signer: "0x00000000000000000000000000000000000000dd" };
    auth.current = { isConnected: true, identity: other };
    // Re-render through a view change so the mocked auth hook is read again.
    await user.click(screen.getByRole("button", { name: "My places" }));
    await user.click(screen.getByRole("button", { name: "Favourites" }));
    await waitFor(() =>
      expect(fetchDestinations).toHaveBeenCalledWith(
        expect.objectContaining({ only_favorites: true }),
        expect.objectContaining({ identity: other }),
      ),
    );
  });

  it("should show the error with Retry when the member read fails", async () => {
    auth.current = { isConnected: true, identity: { signer: MEMBER } };
    serve({ member: new Error("401") });
    renderPage("/places?tab=favourites");
    expect((await screen.findByRole("alert")).textContent).toBe("Something went wrong loading places.");
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
  });
});

describe("when a card is opened", () => {
  it("should open JUMP IN in place for an empty scene on desktop, trap focus and close on Escape", async () => {
    serve({ browse: [destination({ title: "Quiet", description: "Come by" })] });
    const { user } = renderPage();
    const link = await screen.findByRole("link", { name: "Quiet" });
    await user.click(link);
    const dialog = screen.getByRole("dialog", { name: "Quiet" });
    expect(location()).toBe("/places");
    expect(
      within(dialog)
        .getByRole("link", { name: /Jump in/ })
        .getAttribute("href"),
    ).toMatch(/^\/play/);
    expect(within(dialog).getByText("Come by")).toBeTruthy();
    expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: "Close" }));
    await user.tab({ shift: true });
    expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: "Copy link" }));
    await user.tab();
    expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: "Close" }));
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(link);
  });

  it("should confirm a copied link and stay silent when the clipboard refuses", async () => {
    serve({ browse: [destination({ title: "Quiet" })] });
    const { user } = renderPage();
    await user.click(await screen.findByRole("link", { name: "Quiet" }));
    const writeText = vi.fn().mockRejectedValueOnce(new Error("denied")).mockResolvedValueOnce(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    await user.click(screen.getByRole("button", { name: "Copy link" }));
    await waitFor(() => expect(writeText).toHaveBeenCalledOnce());
    expect(screen.queryByText("Copied!")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Copy link" }));
    expect(await screen.findByText("Copied!")).toBeTruthy();
    expect(writeText).toHaveBeenLastCalledWith(`${window.location.origin}/places/plaza`);
  });

  it("should open the place page from Live Now", async () => {
    serve({ browse: [destination({ id: "other", title: "Other" })], live: [destination({ title: "Was busy", user_count: 1 })] });
    const { user } = renderPage();
    const group = await screen.findByRole("group", { name: "Live Now" });
    await user.click(within(group).getByRole("link", { name: "Was busy" }));
    expect(await screen.findByText("Place page")).toBeTruthy();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("should follow the link for a scene with people in it", async () => {
    serve({ browse: [destination({ title: "Busy", user_count: 2 })] });
    const { user } = renderPage("/places?category=music");
    await user.click(await screen.findByRole("link", { name: "Busy" }));
    expect(await screen.findByText("Place page")).toBeTruthy();
  });

  it("should follow the link on a phone instead of opening the modal", async () => {
    serve({ browse: [destination({ title: "Quiet" })] });
    const { user } = renderPage("/places", { kind: "mobile", os: "android" });
    await user.click(await screen.findByRole("link", { name: "Quiet" }));
    expect(await screen.findByText("Place page")).toBeTruthy();
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
