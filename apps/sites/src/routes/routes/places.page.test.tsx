// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ComponentProps, ReactNode } from "react";
import { createRoutesStub, useLocation, Outlet, RouterContextProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { destination } from "@/data/lib/catalyst/places/destination.fixture";
import { fetchDestinations, type Destination, type DestinationParams } from "@/data/lib/catalyst/places/destinations";
import { pageSid } from "@/core/lib/telemetry/session";
import { usePrivateQueryLifecycle } from "@/features/components/chrome/usePrivateQueryLifecycle";
import { openSignIn } from "@/features/components/auth/signin-store";

import { primeRemoteFlags } from "@dcl/one-ui/apps/shop/remote-flags";

import Places, { clientLoader, resetPlacesFlagsRead } from "./places";
import { page, track } from "@/core/lib/telemetry/track";

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
vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn(), page: vi.fn() }));
vi.mock("@/features/components/chrome/SitesLayout", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));
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
  return <output aria-label="Location">{location.pathname + location.search}</output>;
}

function SessionFrame() {
  useLocation();
  usePrivateQueryLifecycle(auth.current.isConnected ? (auth.current.identity?.signer ?? null) : null);
  return <Outlet />;
}

function renderPage(url = "/places", platform: object = DESKTOP, unavailable = false, loaderSid: string | null = "sid", liveMinUsers?: number) {
  const Stub = createRoutesStub([
    {
      id: "root",
      path: "/",
      Component: SessionFrame,
      loader: () => ({
        sid: pageSid(new Request("https://example.org/places", { headers: { cookie: "sid=root-session" } }), new RouterContextProvider()),
      }),
      children: [
        {
          path: "places",
          Component: (props: ComponentProps<typeof Places>) => (
            <>
              <Places {...props} />
              <Location />
            </>
          ),
          loader: () => ({ sid: loaderSid ?? undefined, platform, dehydratedState: undefined, unavailable, liveMinUsers }),
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
  return { user: userEvent.setup(), client };
}

const location = () => screen.getByRole("status", { name: "Location" }).textContent ?? "";

beforeEach(() => {
  resetPlacesFlagsRead();
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

const pageOf = (prefix: string) => Array.from({ length: 48 }, (_, i) => destination({ id: `${prefix}${i}`, title: `Place ${i}` }));
const owner = (n: number) => `0x${n.toString(16).padStart(40, "0")}`;
const LOAD_ERROR = "Something went wrong loading places.";

describe("Places page", () => {
  let user: ReturnType<typeof userEvent.setup>;
  let client: QueryClient;

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
    describe("and Live Now shows one of the feed's places", () => {
      let grid: HTMLElement;

      beforeEach(async () => {
        const busy = destination({ id: "busy", title: "Busy", user_count: 5 });
        serve({ browse: [busy, destination({ id: "quiet", title: "Quiet" })], live: [busy] });
        renderPage();
        await screen.findByRole("link", { name: "Quiet" });
        grid = screen.getByRole("list", { name: "Explore all" });
      });

      it("should list the feed", () => {
        expect(within(grid).getByRole("link", { name: "Quiet" })).toBeTruthy();
      });

      it("should leave out what Live Now already shows", () => {
        expect(within(grid).queryByRole("link", { name: "Busy" })).toBeNull();
      });

      it("should show Live Now", () => {
        expect(screen.getByRole("heading", { name: "Live Now" })).toBeTruthy();
      });
    });

    describe("and the page loader hands no session", () => {
      beforeEach(async () => {
        serve({ browse: [destination({ title: "Quiet" })] });
        ({ user } = renderPage("/places", DESKTOP, false, null));
        await user.click(await screen.findByRole("link", { name: "Quiet" }));
      });

      it("should retain the existing root session for list analytics", () => {
        expect(track).toHaveBeenCalledWith("place_list_viewed", expect.anything(), { sid: "root-session", story: "misc/browse-places" });
      });

      it("should retain the existing root session for click analytics", () => {
        expect(track).toHaveBeenCalledWith("place_card_clicked", expect.anything(), { sid: "root-session", story: "misc/browse-places" });
      });

      it("should send the legacy Places page view once with the root session", () => {
        expect(page).toHaveBeenCalledTimes(1);
        expect(page).toHaveBeenCalledWith("Places — Decentraland", {}, { sid: "root-session" });
      });
    });

    describe("and nothing matches", () => {
      beforeEach(() => {
        serve({ browse: [] });
        renderPage("/places?category=music");
      });

      it("should say so", async () => {
        expect(await screen.findByText("No places match your filters.")).toBeTruthy();
      });
    });

    describe("and the feed fails", () => {
      beforeEach(() => {
        serve({ browse: new Error("Catalyst returned 502") });
        ({ user } = renderPage());
      });

      it("should show the error", async () => {
        expect((await screen.findByRole("alert")).textContent).toBe(LOAD_ERROR);
      });

      describe("and the feed answers again on Retry", () => {
        beforeEach(async () => {
          await screen.findByRole("alert");
          serve({ browse: [destination({ title: "Back" })] });
          await user.click(screen.getByRole("button", { name: "Retry" }));
        });

        it("should recover", async () => {
          expect(await screen.findByRole("link", { name: "Back" })).toBeTruthy();
        });
      });
    });

    describe("and the document answered 503", () => {
      beforeEach(() => {
        serve({ browse: [destination({ title: "Back" })] });
        ({ user } = renderPage("/places", DESKTOP, true));
      });

      it("should show the error", async () => {
        expect((await screen.findByRole("alert")).textContent).toBe(LOAD_ERROR);
      });

      it("should not read again until Retry", async () => {
        await screen.findByRole("alert");
        expect(fetchDestinations).not.toHaveBeenCalledWith(expect.objectContaining({ limit: 48 }), expect.anything());
        expect(screen.queryByText("Loading places…")).toBeNull();
      });

      describe("and the visitor clicks Retry", () => {
        beforeEach(async () => {
          await screen.findByRole("alert");
          await user.click(screen.getByRole("button", { name: "Retry" }));
        });

        it("should read the feed", async () => {
          expect(await screen.findByRole("link", { name: "Back" })).toBeTruthy();
        });
      });
    });

    describe("and the document answered 503 and the view changes", () => {
      let select: HTMLElement;

      beforeEach(async () => {
        serve({ browse: [destination({ title: "Music place" })] });
        ({ user } = renderPage("/places", DESKTOP, true));
        await screen.findByRole("alert");
        [select] = screen.getAllByRole("combobox", { name: "Category" });
        await user.selectOptions(select!, "music");
      });

      it("should read normally", async () => {
        expect(await screen.findByRole("link", { name: "Music place" })).toBeTruthy();
        expect(screen.queryByRole("alert")).toBeNull();
      });

      describe("and the view changes back to all", () => {
        beforeEach(async () => {
          await screen.findByRole("link", { name: "Music place" });
          await user.selectOptions(select!, "all");
        });

        it("should still read normally", async () => {
          await waitFor(() => expect(location()).toBe("/places"));
          expect(await screen.findByRole("link", { name: "Music place" })).toBeTruthy();
          expect(screen.queryByRole("alert")).toBeNull();
        });
      });
    });

    describe("and a whole page is hidden", () => {
      beforeEach(() => {
        serve({ browse: Array.from({ length: 48 }, (_, i) => destination({ id: `e${i}`, title: "Empty" })), total: 100 });
        renderPage();
      });

      it("should offer the next page", async () => {
        expect(await screen.findByRole("button", { name: "Load more places" })).toBeTruthy();
      });

      it("should not claim the catalogue is empty", async () => {
        await screen.findByRole("button", { name: "Load more places" });
        expect(screen.queryByText("No places match your filters.")).toBeNull();
      });
    });

    describe("and the catalogue has more than one page", () => {
      let more: HTMLElement;

      beforeEach(async () => {
        serve({ browse: pageOf("p"), total: 100 });
        ({ user } = renderPage());
        more = await screen.findByRole("button", { name: "Load more places" });
      });

      describe("and the visitor loads more", () => {
        beforeEach(async () => {
          await user.click(more);
        });

        it("should load the next page from the offset after the first", async () => {
          await waitFor(() => expect(fetchDestinations).toHaveBeenCalledWith(expect.objectContaining({ offset: 48 }), expect.anything()));
        });
      });

      describe("and the next page repeats a row", () => {
        beforeEach(async () => {
          serve({ browse: [destination({ id: "p47", title: "Place 47" }), destination({ id: "new", title: "New" })], total: 100 });
          await user.click(more);
          await screen.findByRole("link", { name: "New" });
        });

        it("should list the repeated row once", () => {
          expect(screen.getAllByRole("link", { name: "Place 47" })).toHaveLength(1);
        });
      });

      describe("and the next page fails", () => {
        beforeEach(async () => {
          serve({ browse: new Error("timeout") });
          await user.click(more);
        });

        it("should show the error", async () => {
          expect((await screen.findByRole("alert")).textContent).toBe(LOAD_ERROR);
        });

        it("should keep the loaded cards", async () => {
          await screen.findByRole("alert");
          expect(screen.getByRole("link", { name: "Place 0" })).toBeTruthy();
        });

        describe("and the next page answers on Retry", () => {
          beforeEach(async () => {
            await screen.findByRole("alert");
            serve({ browse: [destination({ id: "next", title: "Next page" })], total: 100 });
            await user.click(screen.getByRole("button", { name: "Retry" }));
          });

          it("should list the next page", async () => {
            expect(await screen.findByRole("link", { name: "Next page" })).toBeTruthy();
          });
        });
      });
    });

    describe("and the places have creators", () => {
      let more: HTMLElement;

      beforeEach(async () => {
        serve({
          browse: Array.from({ length: 48 }, (_, i) => destination({ id: `p${i}`, title: `Place ${i}`, owner: owner(i + 1) })),
          total: 100,
        });
        ({ user } = renderPage());
        more = await screen.findByRole("button", { name: "Load more places" });
        await waitFor(() => expect(fetch).toHaveBeenCalledOnce());
      });

      describe("and the next page adds a new creator", () => {
        beforeEach(async () => {
          serve({ browse: [destination({ id: "p0", owner: owner(1) }), destination({ id: "new", title: "New", owner: owner(99) })], total: 100 });
          await user.click(more);
          await screen.findByRole("link", { name: "New" });
        });

        it("should read the faces of the new page's creators only", async () => {
          await waitFor(() => expect(fetch).toHaveBeenCalledTimes(2));
          const body = JSON.parse(String((vi.mocked(fetch).mock.calls[1] as unknown as [string, RequestInit])[1].body)) as { ids: string[] };
          expect(body.ids).toEqual([owner(99)]);
        });
      });
    });
  });

  describe("when the view changes", () => {
    describe("and the visitor picks a section", () => {
      beforeEach(async () => {
        serve({ browse: [destination()] });
        ({ user } = renderPage());
        await user.click(await screen.findByRole("button", { name: "Favourites" }));
      });

      it("should put the section in ?tab and mark it pressed", async () => {
        await waitFor(() => expect(location()).toBe("/places?tab=favourites"));
        expect(screen.getByRole("button", { name: "Favourites" }).getAttribute("aria-pressed")).toBe("true");
      });

      describe("and then Explore all", () => {
        beforeEach(async () => {
          await waitFor(() => expect(location()).toBe("/places?tab=favourites"));
          await user.click(screen.getByRole("button", { name: "Explore all" }));
        });

        it("should drop ?tab", async () => {
          await waitFor(() => expect(location()).toBe("/places"));
        });
      });
    });

    describe("and the visitor picks a category", () => {
      beforeEach(async () => {
        serve({ browse: [destination()] });
        ({ user } = renderPage());
        const [select] = await screen.findAllByRole("combobox", { name: "Category" });
        await user.selectOptions(select!, "music");
      });

      it("should put the category in ?category", () => {
        expect(location()).toBe("/places?category=music");
      });

      it("should filter the feed", async () => {
        await waitFor(() =>
          expect(fetchDestinations).toHaveBeenCalledWith(expect.objectContaining({ categories: "music" }), expect.anything()),
        );
      });

      describe("and then removes it from the chip", () => {
        beforeEach(async () => {
          await user.click(screen.getByRole("button", { name: "Remove filter: Music" }));
        });

        it("should clear ?category", () => {
          expect(location()).toBe("/places");
        });
      });
    });

    describe("and the visitor types a search", () => {
      beforeEach(async () => {
        serve({ browse: [destination()], live: [destination({ id: "live", user_count: 3 })] });
        ({ user } = renderPage());
      });

      it("should show Live Now before the search", async () => {
        expect(await screen.findByRole("heading", { name: "Live Now" })).toBeTruthy();
      });

      describe("and typing pauses", () => {
        beforeEach(async () => {
          await screen.findByRole("heading", { name: "Live Now" });
          await user.type(screen.getByRole("searchbox", { name: "Search places" }), "plaza");
        });

        it("should put the search in ?search", async () => {
          await waitFor(() => expect(location()).toBe("/places?search=plaza"));
        });

        it("should search the feed and collapse Live Now", async () => {
          await waitFor(() => expect(fetchDestinations).toHaveBeenCalledWith(expect.objectContaining({ search: "plaza" }), expect.anything()));
          expect(screen.queryByRole("heading", { name: "Live Now" })).toBeNull();
        });
      });
    });

    describe("and the search is shorter than three characters", () => {
      beforeEach(async () => {
        serve({ browse: [destination()] });
        renderPage("/places?search=ab");
        await screen.findByRole("list", { name: "Explore all" });
      });

      it("should keep it in the search box", () => {
        expect(screen.getByRole("searchbox", { name: "Search places" })).toHaveProperty("value", "ab");
      });

      it("should not send it", () => {
        expect(fetchDestinations).not.toHaveBeenCalledWith(expect.objectContaining({ search: "ab" }), expect.anything());
      });
    });
  });

  describe("when a member section is open", () => {
    describe("and the visitor is signed out", () => {
      beforeEach(async () => {
        serve({ member: [destination()] });
        ({ user } = renderPage("/places?tab=favourites"));
        await user.click(await screen.findByRole("button", { name: "Sign In" }));
      });

      it("should explain what signing in gives", () => {
        expect(screen.getByText("Sign in and favourite places to find them here.")).toBeTruthy();
      });

      it("should ask them to sign in", () => {
        expect(openSignIn).toHaveBeenCalledOnce();
      });

      it("should read nothing for them", () => {
        expect(fetchDestinations).not.toHaveBeenCalledWith(expect.objectContaining({ only_favorites: true }), expect.anything());
      });
    });

    describe("and the visitor is signed in", () => {
      beforeEach(() => {
        auth.current = { isConnected: true, identity: { signer: MEMBER } };
      });

      describe("and the section is favourites", () => {
        beforeEach(() => {
          serve({ member: [destination({ title: "Saved" })] });
          renderPage("/places?tab=favourites");
        });

        it("should list the member's favourites", async () => {
          expect(await screen.findByRole("link", { name: "Saved" })).toBeTruthy();
        });

        it("should read them signed as the member", async () => {
          await screen.findByRole("link", { name: "Saved" });
          expect(fetchDestinations).toHaveBeenCalledWith(
            expect.objectContaining({ only_favorites: true }),
            expect.objectContaining({ identity: auth.current.identity }),
          );
        });
      });

      describe.each([
        ["favourites", null],
        ["my", null],
        ["my", "https://example.com/v2/map.png"],
        ["favourites", "https://example.com/bafkreidj26s7aenyxfthfdibnqonzqm5ptc4iamml744gmcyuokewkr76y"],
      ])("and the %s section has a row with cover %s", (tab, image) => {
        beforeEach(() => {
          serve({ member: [destination({ title: "Empty", image })] });
          renderPage(`/places?tab=${tab}`);
        });

        it("should preserve the row", async () => {
          expect(await screen.findByRole("link", { name: "Empty" })).toBeTruthy();
        });
      });

      describe("and the section is the member's own places", () => {
        beforeEach(() => {
          serve({ member: [] });
          renderPage("/places?tab=my");
        });

        it("should say they have published none", async () => {
          expect(await screen.findByText("You haven't published any places yet.")).toBeTruthy();
        });

        it("should list them by the member's address", async () => {
          await screen.findByText("You haven't published any places yet.");
          expect(fetchDestinations).toHaveBeenCalledWith(expect.objectContaining({ owner: MEMBER }), expect.anything());
        });
      });

      describe("and a favourite with people in it is opened on desktop", () => {
        beforeEach(async () => {
          serve({ member: [destination({ title: "Busy favourite", user_count: 7 })] });
          ({ user } = renderPage("/places?tab=favourites"));
          await user.click(await screen.findByRole("link", { name: "Busy favourite" }));
        });

        it("should open JUMP IN", () => {
          expect(screen.getByRole("dialog", { name: "Busy favourite" })).toBeTruthy();
        });

        it("should show how many people are in it", () => {
          expect(screen.getByRole("img", { name: "7 people in-world" })).toBeTruthy();
        });
      });

      describe("and the account switches and then signs out", () => {
        beforeEach(async () => {
          serve({ member: [destination({ title: "Old account scene" })] });
          ({ user, client } = renderPage("/places?tab=favourites"));
          await screen.findByRole("link", { name: "Old account scene" });
          client.setQueryData(["private", MEMBER, "places", "mine"], { data: [destination({ title: "Private mine" })] });
          auth.current = { isConnected: true, identity: { signer: "0x00000000000000000000000000000000000000dd" } };
          serve({ member: [destination({ title: "New account scene" })] });
        });

        it("should remove old account caches and rows on account switch and sign-out", async () => {
          await user.click(screen.getByRole("button", { name: "My places" }));
          await screen.findByRole("link", { name: "New account scene" });
          expect(screen.queryByRole("link", { name: "Old account scene" })).toBeNull();
          expect(client.getQueriesData({ queryKey: ["private", MEMBER] })).toEqual([]);
          await user.click(screen.getByRole("button", { name: "Favourites" }));
          await screen.findByRole("link", { name: "New account scene" });
          auth.current = { isConnected: false, identity: null };
          await user.click(screen.getByRole("button", { name: "My places" }));
          await screen.findByRole("button", { name: "Sign In" });
          expect(screen.queryByRole("link", { name: "New account scene" })).toBeNull();
          expect(client.getQueriesData({ queryKey: ["private"] })).toEqual([]);
        });
      });

      describe("and the account switches while a read is pending", () => {
        let signal: AbortSignal | undefined;
        let finish: (page: { data: Destination[]; total: number; received: number; copy: null }) => void;
        let latePage: { data: Destination[]; total: number; received: number; copy: null };

        beforeEach(async () => {
          signal = undefined;
          latePage = { data: [destination({ title: "Late private scene" })], total: 1, received: 1, copy: null };
          vi.mocked(fetchDestinations).mockImplementation((_params, options) => {
            if (!options?.identity) return Promise.resolve({ data: [], total: 0, received: 0, copy: null });
            signal = options.signal;
            return new Promise((resolve) => {
              finish = resolve;
            });
          });
          ({ user, client } = renderPage("/places?tab=favourites"));
          await waitFor(() => expect(signal).toBeDefined());
          auth.current = { isConnected: true, identity: { signer: "0x00000000000000000000000000000000000000dd" } };
          serve({ member: [destination({ title: "Current account" })] });
          await user.click(screen.getByRole("button", { name: "My places" }));
          await screen.findByRole("link", { name: "Current account" });
        });

        it("should abort the old account's pending read", () => {
          expect(signal?.aborted).toBe(true);
        });

        describe("and the old read resolves late", () => {
          beforeEach(async () => {
            await act(async () => finish(latePage));
          });

          it("should not render its result", () => {
            expect(screen.queryByRole("link", { name: "Late private scene" })).toBeNull();
          });

          it("should not cache its result", () => {
            expect(client.getQueriesData({ queryKey: ["private", MEMBER] })).toEqual([]);
          });
        });
      });

      describe("and the member read fails", () => {
        beforeEach(() => {
          serve({ member: new Error("401") });
          renderPage("/places?tab=favourites");
        });

        it("should show the error", async () => {
          expect((await screen.findByRole("alert")).textContent).toBe(LOAD_ERROR);
        });

        it("should offer Retry", async () => {
          await screen.findByRole("alert");
          expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
        });
      });
    });
  });

  describe("when a card is opened", () => {
    describe("and the scene is empty on desktop", () => {
      let link: HTMLElement;
      let dialog: HTMLElement;

      beforeEach(async () => {
        serve({ browse: [destination({ title: "Quiet", description: "Come by" })] });
        ({ user } = renderPage());
        link = await screen.findByRole("link", { name: "Quiet" });
        await user.click(link);
        dialog = screen.getByRole("dialog", { name: "Quiet" });
      });

      it("should open JUMP IN in place", () => {
        expect(location()).toBe("/places");
      });

      it("should link Jump in to /play", () => {
        expect(
          within(dialog)
            .getByRole("link", { name: /Jump in/ })
            .getAttribute("href"),
        ).toMatch(/^\/play/);
      });

      it("should show the description", () => {
        expect(within(dialog).getByText("Come by")).toBeTruthy();
      });

      it("should trap focus and close on Escape, returning focus to the card", async () => {
        expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: "Close" }));
        await user.tab({ shift: true });
        expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: "Copy link" }));
        await user.tab();
        expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: "Close" }));
        await user.keyboard("{Escape}");
        expect(screen.queryByRole("dialog")).toBeNull();
        expect(document.activeElement).toBe(link);
      });
    });

    describe("and the visitor copies its link", () => {
      let writeText: ReturnType<typeof vi.fn>;

      beforeEach(async () => {
        serve({ browse: [destination({ title: "Quiet" })] });
        ({ user } = renderPage());
        await user.click(await screen.findByRole("link", { name: "Quiet" }));
        writeText = vi.fn().mockRejectedValueOnce(new Error("denied")).mockResolvedValueOnce(undefined);
        Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
      });

      it("should stay silent when the clipboard refuses and confirm once it accepts", async () => {
        await user.click(screen.getByRole("button", { name: "Copy link" }));
        await waitFor(() => expect(writeText).toHaveBeenCalledOnce());
        expect(screen.queryByText("Copied!")).toBeNull();
        await user.click(screen.getByRole("button", { name: "Copy link" }));
        expect(await screen.findByText("Copied!")).toBeTruthy();
        expect(writeText).toHaveBeenLastCalledWith(`${window.location.origin}/places/plaza`);
      });
    });

    describe("and it is in Live Now", () => {
      beforeEach(async () => {
        serve({ browse: [destination({ id: "other", title: "Other" })], live: [destination({ title: "Was busy", user_count: 1 })] });
        ({ user } = renderPage());
        const group = await screen.findByRole("group", { name: "Live Now" });
        await user.click(within(group).getByRole("link", { name: "Was busy" }));
      });

      it("should open the place page", async () => {
        expect(await screen.findByText("Place page")).toBeTruthy();
      });

      it("should not open the modal", async () => {
        await screen.findByText("Place page");
        expect(screen.queryByRole("dialog")).toBeNull();
      });
    });

    describe("and the scene has people in it", () => {
      beforeEach(async () => {
        serve({ browse: [destination({ title: "Busy", user_count: 2 })] });
        ({ user } = renderPage("/places?category=music"));
        await user.click(await screen.findByRole("link", { name: "Busy" }));
      });

      it("should follow the link", async () => {
        expect(await screen.findByText("Place page")).toBeTruthy();
      });
    });

    describe("and the visitor is on a phone", () => {
      beforeEach(async () => {
        serve({ browse: [destination({ title: "Quiet" })] });
        ({ user } = renderPage("/places", { kind: "mobile", os: "android" }));
        await user.click(await screen.findByRole("link", { name: "Quiet" }));
      });

      it("should follow the link", async () => {
        expect(await screen.findByText("Place page")).toBeTruthy();
      });

      it("should not open the modal", async () => {
        await screen.findByText("Place page");
        expect(screen.queryByRole("dialog")).toBeNull();
      });
    });
  });
});

describe("when the flags set Live Now's head count", () => {
  it("should leave out scenes under the loader's head count", async () => {
    serve({ browse: [], live: [destination({ id: "busy", title: "Busy", user_count: 6 }), destination({ id: "quiet", title: "Quiet", user_count: 2 })] });
    renderPage("/places", DESKTOP, false, "sid", 5);
    const group = await screen.findByRole("group", { name: "Live Now" });
    expect(within(group).getByRole("link", { name: "Busy" })).toBeTruthy();
    expect(within(group).queryByRole("link", { name: "Quiet" })).toBeNull();
  });

  it("should keep every scene when the loader hands no head count", async () => {
    serve({ browse: [], live: [destination({ id: "plaza", title: "Plaza", user_count: 0 })] });
    renderPage();
    const group = await screen.findByRole("group", { name: "Live Now" });
    expect(within(group).getByRole("link", { name: "Plaza" })).toBeTruthy();
  });
});

describe("when navigating to /places in the browser", () => {
  const FLAG = "dapps-places-live-min-user";

  beforeEach(() => {
    resetPlacesFlagsRead();
    primeRemoteFlags(null);
  });

  it("should read the head count from this server's flags once per document", async () => {
    const fetchMock = vi.fn(async () => Response.json({ flags: { [FLAG]: true }, variants: { [FLAG]: "5" } }));
    vi.stubGlobal("fetch", fetchMock);
    await expect(clientLoader()).resolves.toEqual({ liveMinUsers: 5 });
    await expect(clientLoader()).resolves.toEqual({ liveMinUsers: 5 });
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock).toHaveBeenCalledWith("/internal/feature-flags?variants=1", expect.anything());
  });

  it("should hand 0 while the flag is off", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => Response.json({ flags: { [FLAG]: false }, variants: { [FLAG]: "5" } })));
    await expect(clientLoader()).resolves.toEqual({ liveMinUsers: 0 });
  });

  describe("and the server already rendered this document with a head count", () => {
    let fetchMock: ReturnType<typeof vi.fn>;

    beforeEach(async () => {
      serve({ browse: [], live: [destination({ id: "busy", title: "Busy", user_count: 6 })] });
      renderPage("/places", DESKTOP, false, "sid", 5);
      await screen.findByRole("group", { name: "Live Now" });
      fetchMock = vi.fn(async () => new Response("down", { status: 503 }));
      vi.stubGlobal("fetch", fetchMock);
    });

    it("should keep the server's head count on the next navigation without reading the flags", async () => {
      await expect(clientLoader()).resolves.toEqual({ liveMinUsers: 5 });
      expect(fetchMock).not.toHaveBeenCalled();
    });
  });

  it("should keep the head count of a successful read when a later navigation follows", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => Response.json({ flags: { [FLAG]: true }, variants: { [FLAG]: "5" } })));
    await expect(clientLoader()).resolves.toEqual({ liveMinUsers: 5 });
    const failing = vi.fn(async () => new Response("down", { status: 503 }));
    vi.stubGlobal("fetch", failing);
    await expect(clientLoader()).resolves.toEqual({ liveMinUsers: 5 });
    expect(failing).not.toHaveBeenCalled();
  });

  it("should hand 0 and not ask again on the next navigation when the flags cannot be read", async () => {
    const fetchMock = vi.fn(async () => new Response("down", { status: 503 }));
    vi.stubGlobal("fetch", fetchMock);
    await expect(clientLoader()).resolves.toEqual({ liveMinUsers: 0 });
    await expect(clientLoader()).resolves.toEqual({ liveMinUsers: 0 });
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });
});
