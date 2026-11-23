// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});

const net = vi.hoisted(() => ({
  fetchCommunity: vi.fn(),
  fetchCommunityMembers: vi.fn(),
  fetchCommunityEvents: vi.fn(),
  fetchJoinRequests: vi.fn(),
  joinCommunity: vi.fn(),
  requestToJoin: vi.fn(),
  cancelJoinRequest: vi.fn(),
}));
const auth = vi.hoisted(() => ({ identity: null as null | { signer: string } }));
const signIn = vi.hoisted(() => ({ openSignIn: vi.fn() }));
const reminders = vi.hoisted(() => ({ failed: null as string | null, onSignIn: null as null | (() => void) }));
const events = vi.hoisted(() => ({ fetchEvent: vi.fn() }));
const telemetry = vi.hoisted(() => ({ track: vi.fn() }));

vi.mock("@/data/lib/catalyst/social/community-detail", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/social/community-detail")>()),
  ...net,
}));
vi.mock("@/data/lib/catalyst/places/events", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/places/events")>()),
  ...events,
}));
vi.mock("@/core/lib/telemetry/track", () => telemetry);
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => ({ identity: auth.identity }) }));
vi.mock("@/features/components/auth/signin-store", () => signIn);
vi.mock("@/features/components/chrome/SitesLayout", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));
const creators = vi.hoisted(() => ({ asked: [] as string[] }));
vi.mock("@/features/events/useCreatorProfiles", () => ({
  useCreatorProfiles: (groups: string[][]) => {
    creators.asked.push(...groups.flat());
    return {};
  },
}));
vi.mock("@/features/events/useEventReminders", () => ({
  REMIND_FAILED: "Couldn't update your reminder. Try again.",
  useEventReminders: (_identity: unknown, onSignIn: () => void) => {
    reminders.onSignIn = onSignIn;
    return { failed: reminders.failed, get: () => ({ active: false, pending: false, toggle: () => {} }) };
  },
}));

import { CatalystError } from "@/data/lib/catalyst/client";
import type { Event } from "@/data/lib/catalyst/places/events";
import { eventsPageKeys } from "@/data/lib/catalyst/places/events-page";
import type { CommunityInfo } from "@/data/lib/catalyst/social/community-detail";
import { communityKeys } from "@/data/lib/community/queries";

import { CommunityPage } from "./CommunityPage";
import { PENDING_ACTION_TTL_MS, rememberAction } from "@/features/social/community/pendingAction";

const ID = "e99471aa-31c4-4952-abf6-99905445f43b";
const SID = "sid-123";
const ME = "0x1111111111111111111111111111111111111111";

const community = (over: Partial<CommunityInfo> = {}): CommunityInfo => ({
  id: ID,
  name: "Decentraland Foundation",
  description: "Official updates.",
  ownerAddress: "0xbcac4dafb7e215f2f6cb3312af6d5e4f9d9e7eda",
  ownerName: "DCLOfficial",
  privacy: "public",
  membersCount: 1340,
  thumbnailUrl: null,
  role: null,
  ...over,
});

const upcomingEvent = (): Event => ({
  id: "ev-1",
  name: "Takeover Tuesday",
  image: null,
  image_vertical: null,
  description: "Rave classics all night.",
  start_at: "2099-06-01T18:00:00.000Z",
  finish_at: "2099-06-01T19:00:00.000Z",
  next_start_at: "2099-06-01T18:00:00.000Z",
  all_day: false,
  x: 0,
  y: 5,
  position: [0, 5],
  coordinates: [0, 5],
  url: null,
  user_name: "Decentraland Foundation",
  scene_name: "Genesis Plaza",
  estate_name: null,
  live: false,
  highlighted: false,
  trending: false,
  recurrent: false,
  recurrent_frequency: null,
  total_attendees: 0,
  place_id: null,
});

let client: QueryClient;

type Entry = string | { pathname: string; search: string; state: unknown };

function show(path: Entry = `/social/communities/${ID}`, sid = SID, seed?: (client: QueryClient) => void) {
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  seed?.(client);
  const router = createMemoryRouter(
    [{ path: "/social/communities/:id", element: <CommunityPage id={ID} now={0} sid={sid} /> }],
    { initialEntries: [path] },
  );
  render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return router;
}

/** The anonymous read, then the member's own read with `role`. */
function serve(anonymous: CommunityInfo, viewer?: Partial<CommunityInfo>) {
  net.fetchCommunity.mockImplementation(async (_id: string, opts: { identity?: unknown } = {}) =>
    opts.identity ? { ...anonymous, ...viewer } : anonymous,
  );
}

beforeEach(() => {
  for (const fn of Object.values(net)) fn.mockReset();
  events.fetchEvent.mockReset();
  telemetry.track.mockReset();
  creators.asked = [];
  signIn.openSignIn.mockReset();
  auth.identity = null;
  reminders.failed = null;
  reminders.onSignIn = null;
  net.fetchCommunityMembers.mockResolvedValue({
    members: [{ memberAddress: ME, role: "owner", name: "Alice", profilePictureUrl: null, hasClaimedName: true }],
    total: 1,
    hasMore: false,
  });
  net.fetchCommunityEvents.mockResolvedValue({ events: [], total: 0 });
  net.fetchJoinRequests.mockResolvedValue([]);
});
afterEach(() => {
  cleanup();
  sessionStorage.clear();
  vi.resetAllMocks();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

const redirectOf = () => new URL(signIn.openSignIn.mock.calls[0]![0].redirectTo, "http://localhost");

type Member = { memberAddress: string; role: string; name: string; profilePictureUrl: null; hasClaimedName: boolean };
type Router = ReturnType<typeof show>;

describe("CommunityPage", () => {
  describe("when the visitor is signed out", () => {
    describe("and the community is public", () => {
      beforeEach(() => {
        serve(community());
      });

      describe("and the page loads", () => {
        beforeEach(() => {
          show();
        });

        it("should show the community's card, members and events", async () => {
          expect(await screen.findByRole("heading", { level: 1, name: "Decentraland Foundation" })).toBeTruthy();
          expect(screen.getByText("1.3K Members")).toBeTruthy();
          expect(screen.getByText("Official updates.")).toBeTruthy();
          expect(await screen.findByText("Alice")).toBeTruthy();
          expect(screen.getByText("Owner")).toBeTruthy();
          expect(await screen.findByText("No Upcoming Events")).toBeTruthy();
          expect(screen.getByRole("tab", { name: "Members", selected: true })).toBeTruthy();
        });

        describe("and the visitor clicks Sign in to join", () => {
          let back: URL;

          beforeEach(async () => {
            await userEvent.click(await screen.findByRole("button", { name: "Sign in to join" }));
            back = redirectOf();
          });

          it("should send the visitor back to the community page", () => {
            expect(back.pathname).toBe(`/social/communities/${ID}`);
          });

          it("should come back with the join action", () => {
            expect(back.searchParams.get("action")).toBe("join");
          });

          it("should come back with a nonce", () => {
            expect(back.searchParams.get("nonce")).toBeTruthy();
          });
        });

        describe("and the visitor clicks the Upcoming Events tab", () => {
          beforeEach(async () => {
            await userEvent.click(await screen.findByRole("tab", { name: "Upcoming Events" }));
          });

          it("should switch from the members to the events", () => {
            expect(screen.getByRole("tab", { name: "Upcoming Events", selected: true })).toBeTruthy();
            expect(screen.getByRole("tab", { name: "Members", selected: false })).toBeTruthy();
          });
        });
      });

      describe("and the tab cannot record the action", () => {
        beforeEach(() => {
          vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
            throw new Error("QuotaExceededError");
          });
          show();
        });

        it("should send the visitor to sign in and back to the plain page", async () => {
          await userEvent.click(await screen.findByRole("button", { name: "Sign in to join" }));

          expect(signIn.openSignIn).toHaveBeenCalledWith({ redirectTo: `/social/communities/${ID}` });
        });
      });

      describe("and the link carries an unknown action", () => {
        let router: Router;

        beforeEach(() => {
          router = show(`/social/communities/${ID}?action=leave`);
        });

        it("should drop the action", async () => {
          await waitFor(() => expect(router.state.location.search).toBe(""));
        });
      });

      describe("and the link's unknown action is dropped from an entry with state", () => {
        let router: Router;

        beforeEach(() => {
          router = show({ pathname: `/social/communities/${ID}`, search: "?action=leave&event=ev-1", state: { kept: true } });
        });

        it("should keep the entry's state and the other parameters", async () => {
          await waitFor(() => expect(router.state.location.search).toBe("?event=ev-1"));
          expect(router.state.location.state).toEqual({ kept: true });
        });
      });

      describe("and the visitor opens an event on a sign-in return that is still pending", () => {
        let router: Router;
        let entry: string;

        beforeEach(async () => {
          net.fetchCommunityEvents.mockResolvedValue({ events: [upcomingEvent()], total: 1, received: 1 });
          const nonce = rememberAction(ID, "join");
          entry = `?action=join&nonce=${nonce}`;
          router = show(`/social/communities/${ID}${entry}`);
          await userEvent.click(await screen.findByRole("link", { name: "Takeover Tuesday" }));
        });

        it("should name only the event in the detail's URL", async () => {
          await screen.findByRole("dialog");
          expect(router.state.location.search).toBe("?event=ev-1");
        });

        describe("and closes the detail", () => {
          beforeEach(async () => {
            await userEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: /close/i }));
          });

          it("should go back to the sign-in return, which still carries its action", async () => {
            await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
            expect(router.state.location.search).toBe(entry);
          });
        });
      });

      describe("and the link is a sign-in return with an action", () => {
        let nonce: string | null;
        let router: Router;

        beforeEach(() => {
          nonce = rememberAction(ID, "join");
          router = show(`/social/communities/${ID}?action=join&nonce=${nonce}`);
        });

        it("should keep the action while the visitor is still signed out", async () => {
          expect(await screen.findByRole("button", { name: "Sign in to join" })).toBeTruthy();
          expect(router.state.location.search).toBe(`?action=join&nonce=${nonce}`);
          expect(sessionStorage.length).toBe(1);
        });
      });

      describe("and the members cannot be read", () => {
        beforeEach(() => {
          net.fetchCommunityMembers.mockRejectedValue(new Error("502"));
          show();
        });

        it("should say so inline, keeping the card", async () => {
          expect(await screen.findByText("We couldn't load the members right now.")).toBeTruthy();
          expect(screen.getByRole("heading", { level: 1, name: "Decentraland Foundation" })).toBeTruthy();
        });
      });

      describe("and the first members page fails once", () => {
        beforeEach(() => {
          net.fetchCommunityMembers.mockRejectedValueOnce(new Error("502"));
          show();
        });

        it("should offer Try again and show the members once they load", async () => {
          expect(await screen.findByText("We couldn't load the members right now.")).toBeTruthy();
          await userEvent.click(screen.getByRole("button", { name: "Try again" }));
          expect(await screen.findByText("Alice")).toBeTruthy();
        });
      });

      describe("and the first events page fails once", () => {
        beforeEach(() => {
          net.fetchCommunityEvents.mockRejectedValueOnce(new Error("502"));
          show();
        });

        it("should offer Try again and show the events once they load", async () => {
          expect(await screen.findByText("We couldn't load the events right now.")).toBeTruthy();
          await userEvent.click(screen.getByRole("button", { name: "Try again" }));
          expect(await screen.findByText("No Upcoming Events")).toBeTruthy();
        });
      });

      describe("and the community has an upcoming event", () => {
        let router: Router;

        beforeEach(() => {
          net.fetchCommunityEvents.mockResolvedValue({ events: [upcomingEvent()], total: 1, received: 1 });
        });

        describe("and the visitor clicks its card", () => {
          beforeEach(async () => {
            router = show();
            await userEvent.click(await screen.findByRole("link", { name: "Takeover Tuesday" }));
          });

          it("should open the event's detail over the community page and name it in the URL", async () => {
            const dialog = await screen.findByRole("dialog");
            expect(within(dialog).getByText("Rave classics all night.")).toBeTruthy();
            expect(router.state.location.pathname).toBe(`/social/communities/${ID}`);
            expect(router.state.location.search).toBe("?event=ev-1");
          });

          it("should render the detail outside the page's stacking context", async () => {
            const dialog = await screen.findByRole("dialog");
            const heading = screen.getByRole("heading", { level: 1, name: "Decentraland Foundation" });
            const frame = Array.from(document.body.firstElementChild!.children).find((child) => child.contains(heading))!;
            expect(frame.contains(dialog)).toBe(false);
          });

          it("should record the view once, under the events page's story and the visitor's session", async () => {
            await screen.findByRole("dialog");
            await act(async () => {});
            const views = telemetry.track.mock.calls.filter(([name]) => name === "lp_event_viewed");
            expect(views).toEqual([["lp_event_viewed", { event_id: "ev-1", live: false }, { sid: SID, story: "admin/whats-on-detail" }]]);
          });

          describe("and jumps in", () => {
            // Keeps jsdom from following the link.
            const stay = (click: MouseEvent) => click.preventDefault();

            beforeEach(async () => {
              window.addEventListener("click", stay);
              const dialog = await screen.findByRole("dialog");
              await userEvent.click(within(dialog).getAllByRole("link", { name: /Jump In/ })[0]!);
            });

            afterEach(() => {
              window.removeEventListener("click", stay);
            });

            it("should record the Jump In once", () => {
              const jumps = telemetry.track.mock.calls.filter(([name]) => name === "lp_event_jump_in");
              expect(jumps).toEqual([
                ["lp_event_jump_in", { event_id: "ev-1", position: "(0,5)" }, { sid: SID, story: "admin/whats-on-detail" }],
              ]);
            });
          });

          describe("and closes the detail", () => {
            beforeEach(async () => {
              await userEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: /close/i }));
            });

            it("should go back to the community page as it was", async () => {
              await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
              expect(router.state.location.search).toBe("");
              expect(router.state.historyAction).toBe("POP");
              expect(screen.getByRole("heading", { level: 1, name: "Decentraland Foundation" })).toBeTruthy();
            });
          });

          describe("and goes back", () => {
            beforeEach(async () => {
              await screen.findByRole("dialog");
              await act(() => router.navigate(-1));
            });

            it("should close the detail and stay on the community page", async () => {
              await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
              expect(router.state.location.pathname).toBe(`/social/communities/${ID}`);
              expect(router.state.location.search).toBe("");
              expect(screen.getByRole("heading", { level: 1, name: "Decentraland Foundation" })).toBeTruthy();
            });
          });
        });

        describe("and the visitor clicks its card twice before the detail covers it", () => {
          beforeEach(async () => {
            router = show();
            const link = await screen.findByRole("link", { name: "Takeover Tuesday" });
            const plain = () => new MouseEvent("click", { bubbles: true, cancelable: true, button: 0 });
            await act(async () => {
              link.dispatchEvent(plain());
            });
            await act(async () => {
              link.dispatchEvent(plain());
            });
            await userEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: /close/i }));
          });

          it("should close the detail with one Close", async () => {
            await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
            expect(router.state.location.search).toBe("");
          });
        });

        describe("and the page has no session", () => {
          beforeEach(async () => {
            show(`/social/communities/${ID}`, "");
            await userEvent.click(await screen.findByRole("link", { name: "Takeover Tuesday" }));
          });

          it("should open the detail and record nothing", async () => {
            expect(await screen.findByRole("dialog")).toBeTruthy();
            await act(async () => {});
            expect(telemetry.track).not.toHaveBeenCalled();
          });

          describe("and the visitor jumps in", () => {
            // Keeps jsdom from following the link.
            const stay = (click: MouseEvent) => click.preventDefault();

            beforeEach(async () => {
              window.addEventListener("click", stay);
              await userEvent.click(within(await screen.findByRole("dialog")).getAllByRole("link", { name: /Jump In/ })[0]!);
            });

            afterEach(() => {
              window.removeEventListener("click", stay);
            });

            it("should record no Jump In", () => {
              expect(telemetry.track).not.toHaveBeenCalled();
            });
          });
        });

        describe.each([
          ["Ctrl", { ctrlKey: true }],
          ["Cmd", { metaKey: true }],
          ["Shift", { shiftKey: true }],
          ["Alt", { altKey: true }],
          ["the middle button", { button: 1 }],
        ])("and the visitor opens its card with %s", (_name, modifier) => {
          let prevented: boolean | undefined;
          // Runs after the page's handlers: records what they did, then keeps jsdom from navigating.
          const browser = (click: MouseEvent) => {
            prevented = click.defaultPrevented;
            click.preventDefault();
          };

          beforeEach(async () => {
            prevented = undefined;
            window.addEventListener("click", browser);
            router = show();
            const link = await screen.findByRole("link", { name: "Takeover Tuesday" });
            expect(link.getAttribute("href")).toBe("/events?id=ev-1");
            link.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true, button: 0, ...modifier }));
            await act(async () => {});
          });

          afterEach(() => {
            window.removeEventListener("click", browser);
          });

          it("should leave the link to the browser and open no detail", () => {
            expect(prevented).toBe(false);
            expect(screen.queryByRole("dialog")).toBeNull();
            expect(router.state.location.search).toBe("");
          });
        });

        describe("and the link names the event", () => {
          beforeEach(() => {
            router = show(`/social/communities/${ID}?event=ev-1`);
          });

          it("should open its detail without reading it again", async () => {
            const dialog = await screen.findByRole("dialog");
            expect(within(dialog).getByText("Rave classics all night.")).toBeTruthy();
            expect(events.fetchEvent).not.toHaveBeenCalled();
          });

          describe("and the visitor closes the detail", () => {
            beforeEach(async () => {
              await userEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: /close/i }));
            });

            it("should drop the event from the URL, staying on the community page", async () => {
              await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
              expect(router.state.location.pathname).toBe(`/social/communities/${ID}`);
              expect(router.state.location.search).toBe("");
              expect(router.state.historyAction).toBe("REPLACE");
            });
          });
        });

        describe("and the history entry was marked by an earlier document, as after a reload", () => {
          beforeEach(async () => {
            router = show({
              pathname: `/social/communities/${ID}`,
              search: "?event=ev-1",
              state: { communityEventOpened: "an-earlier-document" },
            });
            await userEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: /close/i }));
          });

          it("should close in place, dropping the event from the URL", async () => {
            await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
            expect(router.state.historyAction).toBe("REPLACE");
            expect(router.state.location.search).toBe("");
          });
        });

        describe("and the link names an event the list has not loaded", () => {
          beforeEach(() => {
            events.fetchEvent.mockResolvedValue({ ...upcomingEvent(), id: "ev-9", name: "Later Party", community_id: ID });
            router = show(`/social/communities/${ID}?event=ev-9`);
          });

          it("should read it by id and open its detail", async () => {
            const dialog = await screen.findByRole("dialog");
            expect(within(dialog).getByRole("heading", { name: "Later Party" })).toBeTruthy();
            expect(events.fetchEvent).toHaveBeenCalledWith("ev-9", expect.anything());
            expect(router.state.location.search).toBe("?event=ev-9");
          });
        });

        describe("and the link names another community's event", () => {
          beforeEach(() => {
            events.fetchEvent.mockResolvedValue({ ...upcomingEvent(), id: "ev-9", community_id: "another" });
            router = show(`/social/communities/${ID}?event=ev-9`);
          });

          it("should open no detail and drop the event from the URL", async () => {
            await waitFor(() => expect(router.state.location.search).toBe(""));
            expect(screen.queryByRole("dialog")).toBeNull();
          });
        });

        describe("and the link names an event the catalogue does not know", () => {
          beforeEach(() => {
            events.fetchEvent.mockRejectedValue(new CatalystError("not found", "/api/events/garbage", 404));
            router = show(`/social/communities/${ID}?event=garbage`);
          });

          it("should open no detail and drop the event from the URL", async () => {
            await waitFor(() => expect(router.state.location.search).toBe(""));
            expect(screen.queryByRole("dialog")).toBeNull();
            expect(screen.getByRole("heading", { level: 1, name: "Decentraland Foundation" })).toBeTruthy();
          });
        });

        describe("and the event cannot be read", () => {
          beforeEach(() => {
            events.fetchEvent.mockRejectedValue(new CatalystError("unavailable", "/api/events/ev-9", 503));
            router = show(`/social/communities/${ID}?event=ev-9`);
          });

          it("should open no detail and keep the link", async () => {
            await waitFor(() => expect(events.fetchEvent).toHaveBeenCalled());
            await act(async () => {});
            expect(screen.queryByRole("dialog")).toBeNull();
            expect(router.state.location.search).toBe("?event=ev-9");
          });
        });

        describe("and the link's event parameter is empty", () => {
          beforeEach(() => {
            router = show(`/social/communities/${ID}?event=%20`);
          });

          it("should open no detail and read nothing", async () => {
            expect(await screen.findByRole("link", { name: "Takeover Tuesday" })).toBeTruthy();
            await act(async () => {});
            expect(screen.queryByRole("dialog")).toBeNull();
            expect(events.fetchEvent).not.toHaveBeenCalled();
          });
        });
      });

      describe("and the community has no members", () => {
        beforeEach(() => {
          net.fetchCommunityMembers.mockResolvedValue({ members: [], total: 0, hasMore: false });
          show();
        });

        it("should show no members scroller", async () => {
          expect(await screen.findByText("No members found")).toBeTruthy();
          expect(screen.queryByRole("group", { name: /Members/ })).toBeNull();
        });
      });

      describe("and the members list has a next page", () => {
        let first: Member;
        let second: Member;

        beforeEach(() => {
          first = { memberAddress: ME, role: "owner", name: "Alice", profilePictureUrl: null, hasClaimedName: true };
          second = { ...first, memberAddress: "0x2222222222222222222222222222222222222222", name: "Bob", role: "member" };
        });

        describe("and the next page fails while its sentinel is always on screen", () => {
          beforeEach(() => {
            // The sentinel is always on screen here, as on a short list.
            class Visible {
              constructor(private readonly cb: IntersectionObserverCallback) {}
              observe() {
                this.cb([{ isIntersecting: true } as IntersectionObserverEntry], this as unknown as IntersectionObserver);
              }
              disconnect() {}
            }
            vi.stubGlobal("IntersectionObserver", Visible);
            net.fetchCommunityMembers.mockImplementation(async (_id: string, offset: number) => {
              if (offset === 0) return { members: [first], total: 2, hasMore: true };
              throw new Error("502");
            });
            show();
          });

          it("should ask once and offer Try again, keeping the first page", async () => {
            expect(await screen.findByText("We couldn't load more members.")).toBeTruthy();
            await new Promise((resolve) => setTimeout(resolve, 50));
            expect(net.fetchCommunityMembers).toHaveBeenCalledTimes(2);
            expect(screen.getByText("Alice")).toBeTruthy();

            net.fetchCommunityMembers.mockImplementation(async (_id: string, offset: number) =>
              offset === 0 ? { members: [first], total: 2, hasMore: true } : { members: [second], total: 2, hasMore: false },
            );
            await userEvent.click(screen.getByRole("button", { name: "Try again" }));
            expect(await screen.findByText("Bob")).toBeTruthy();
          });
        });

        describe("and the browser records the observers' roots", () => {
          let roots: (Element | Document | null | undefined)[];

          beforeEach(() => {
            roots = [];
            const recorded = roots;
            class Recorded {
              constructor(_cb: IntersectionObserverCallback, options?: IntersectionObserverInit) {
                recorded.push(options?.root);
              }
              observe() {}
              disconnect() {}
            }
            vi.stubGlobal("IntersectionObserver", Recorded);
            net.fetchCommunityMembers.mockResolvedValue({ members: [first], total: 2, hasMore: true });
            show();
          });

          it("should keep the members in their own scroller and watch its end, not the page's", async () => {
            const list = await screen.findByRole("group", { name: "Members (2)" });
            expect(list.tabIndex).toBe(0);
            expect(within(list).getByText("Alice")).toBeTruthy();
            await waitFor(() => expect(roots).toContain(list));
          });
        });

        describe("and the browser has no IntersectionObserver", () => {
          beforeEach(() => {
            vi.stubGlobal("IntersectionObserver", undefined);
            net.fetchCommunityMembers.mockImplementation(async (_id: string, offset: number) =>
              offset === 0 ? { members: [first], total: 2, hasMore: true } : { members: [second], total: 2, hasMore: false },
            );
            show();
          });

          it("should offer Load more instead of loading on scroll", async () => {
            await userEvent.click(await screen.findByRole("button", { name: "Load more" }));
            expect(await screen.findByText("Bob")).toBeTruthy();
            expect(screen.queryByRole("button", { name: "Load more" })).toBeNull();
          });
        });
      });

      describe("and a Remind me toggle fails", () => {
        beforeEach(() => {
          reminders.failed = "e1";
          show();
        });

        it("should say so", async () => {
          expect(await screen.findByText("Couldn't update your reminder. Try again.")).toBeTruthy();
        });
      });
    });

    describe("and the community is private", () => {
      beforeEach(() => {
        serve(community({ privacy: "private" }));
        show();
      });

      it("should ask the sign-in to come back with a request to join", async () => {
        await userEvent.click(await screen.findByRole("button", { name: "Sign in to join" }));

        expect(redirectOf().searchParams.get("action")).toBe("requestToJoin");
      });
    });
  });

  describe("when the visitor is signed in", () => {
    beforeEach(() => {
      auth.identity = { signer: ME };
    });

    describe("and the visitor is not a member of a public community", () => {
      beforeEach(() => {
        serve(community(), { role: "none" });
      });

      describe("and joining succeeds", () => {
        let role: CommunityInfo["role"];

        beforeEach(() => {
          role = "none";
          net.fetchCommunity.mockImplementation(async (_id: string, opts: { identity?: unknown } = {}) =>
            community(opts.identity ? { role } : {}),
          );
          net.joinCommunity.mockImplementation(async () => {
            role = "member";
          });
          show();
        });

        it("should join the community and show the member as joined", async () => {
          await userEvent.click(await screen.findByRole("button", { name: "Join" }));

          expect(await screen.findByRole("button", { name: "Joined" })).toBeTruthy();
          expect(net.joinCommunity).toHaveBeenCalledWith(auth.identity, ID);
          expect(screen.queryByRole("alert")).toBeNull();
        });
      });

      describe("and joining fails", () => {
        let router: Router;

        beforeEach(async () => {
          net.joinCommunity.mockRejectedValue(new Error("500"));
          router = show();
          await userEvent.click(await screen.findByRole("button", { name: "Join" }));
        });

        it("should keep the member out and say so", async () => {
          expect((await screen.findByRole("alert")).textContent).toBe("Failed to join community");
          expect(screen.getByRole("button", { name: "Join" })).toHaveProperty("disabled", false);
          expect(screen.queryByRole("button", { name: "Joined" })).toBeNull();
          // Nothing was re-read: the member's standing is what it was.
          expect(net.fetchCommunity).toHaveBeenCalledTimes(2);
        });

        describe("and another account signs in", () => {
          it("should drop the failed join's message", async () => {
            expect((await screen.findByRole("alert")).textContent).toBe("Failed to join community");

            auth.identity = { signer: "0x3333333333333333333333333333333333333333" };
            await router.navigate(`/social/communities/${ID}?again=1`);

            await waitFor(() => expect(screen.queryByText("Failed to join community")).toBeNull());
          });
        });
      });

      describe("and the visitor opens an event while the sign-in return's join is pending", () => {
        let router: Router;
        let release: (value: CommunityInfo) => void;

        beforeEach(async () => {
          release = () => {};
          net.fetchCommunity.mockImplementation((_id: string, opts: { identity?: unknown } = {}) =>
            opts.identity
              ? new Promise<CommunityInfo>((resolve) => (release = resolve))
              : Promise.resolve(community()),
          );
          net.fetchCommunityEvents.mockResolvedValue({ events: [upcomingEvent()], total: 1, received: 1 });
          net.joinCommunity.mockResolvedValue(undefined);
          const nonce = rememberAction(ID, "join");
          router = show(`/social/communities/${ID}?action=join&nonce=${nonce}`);
          await userEvent.click(await screen.findByRole("link", { name: "Takeover Tuesday" }));
          await screen.findByRole("dialog");
          await act(async () => release(community({ role: "none" })));
        });

        it("should not join while the detail is open", async () => {
          await act(async () => {});
          expect(net.joinCommunity).not.toHaveBeenCalled();
          expect(router.state.location.search).toBe("?event=ev-1");
        });

        describe("and closes the detail", () => {
          beforeEach(async () => {
            await userEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: /close/i }));
          });

          it("should join once, back on the community page with a clean URL", async () => {
            await waitFor(() => expect(net.joinCommunity).toHaveBeenCalledTimes(1));
            await waitFor(() => expect(router.state.location.search).toBe(""));
            expect(screen.queryByRole("dialog")).toBeNull();
            expect(net.joinCommunity).toHaveBeenCalledTimes(1);
          });
        });
      });

      describe("and the link is this tab's sign-in return with the join action", () => {
        let router: Router;

        beforeEach(() => {
          net.joinCommunity.mockResolvedValue(undefined);
          const nonce = rememberAction(ID, "join");
          router = show(`/social/communities/${ID}?action=join&nonce=${nonce}`);
        });

        it("should run the join once and consume its marker", async () => {
          await waitFor(() => expect(net.joinCommunity).toHaveBeenCalledTimes(1));
          await waitFor(() => expect(router.state.location.search).toBe(""));
          expect(net.joinCommunity).toHaveBeenCalledTimes(1);
          expect(sessionStorage.length).toBe(0);
        });
      });

      describe("and the link is a crafted one this tab did not start", () => {
        let router: Router;

        beforeEach(() => {
          router = show(`/social/communities/${ID}?action=join&nonce=guessed`);
        });

        it("should not join", async () => {
          await waitFor(() => expect(router.state.location.search).toBe(""));
          expect(net.joinCommunity).not.toHaveBeenCalled();
          expect(screen.getByRole("button", { name: "Join" })).toBeTruthy();
        });
      });

      describe("and the link has no nonce", () => {
        let router: Router;

        beforeEach(() => {
          rememberAction(ID, "join");
          router = show(`/social/communities/${ID}?action=join`);
        });

        it("should not join", async () => {
          await waitFor(() => expect(router.state.location.search).toBe(""));
          expect(net.joinCommunity).not.toHaveBeenCalled();
        });
      });

      describe("and the marker has expired", () => {
        let router: Router;

        beforeEach(() => {
          const nonce = rememberAction(ID, "join", sessionStorage, Date.now() - PENDING_ACTION_TTL_MS - 1);
          router = show(`/social/communities/${ID}?action=join&nonce=${nonce}`);
        });

        it("should not join", async () => {
          await waitFor(() => expect(router.state.location.search).toBe(""));
          expect(net.joinCommunity).not.toHaveBeenCalled();
        });
      });

      describe("and the marker is for another community", () => {
        let router: Router;

        beforeEach(() => {
          const nonce = rememberAction("00000000-0000-0000-0000-000000000000", "join");
          router = show(`/social/communities/${ID}?action=join&nonce=${nonce}`);
        });

        it("should not join", async () => {
          await waitFor(() => expect(router.state.location.search).toBe(""));
          expect(net.joinCommunity).not.toHaveBeenCalled();
        });
      });

      describe("and the tab's storage cannot be read", () => {
        let router: Router;

        beforeEach(() => {
          const nonce = rememberAction(ID, "join");
          vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
            throw new Error("SecurityError");
          });
          router = show(`/social/communities/${ID}?action=join&nonce=${nonce}`);
        });

        it("should not join", async () => {
          await waitFor(() => expect(router.state.location.search).toBe(""));
          expect(net.joinCommunity).not.toHaveBeenCalled();
        });
      });
    });

    describe("and the visitor is already a member of a public community", () => {
      describe("and the link is a sign-in return with the join action", () => {
        let router: Router;

        beforeEach(() => {
          serve(community(), { role: "member" });
          const nonce = rememberAction(ID, "join");
          router = show(`/social/communities/${ID}?action=join&nonce=${nonce}`);
        });

        it("should drop the action without running it", async () => {
          await waitFor(() => expect(router.state.location.search).toBe(""));
          expect(net.joinCommunity).not.toHaveBeenCalled();
          expect(sessionStorage.length).toBe(0);
        });
      });
    });

    describe("and the visitor is not a member of a private community", () => {
      beforeEach(() => {
        serve(community({ privacy: "private" }), { role: "none" });
      });

      describe("and the page loads", () => {
        beforeEach(() => {
          show();
        });

        it("should hide the members, events and description", async () => {
          expect(await screen.findByRole("heading", { name: "This is a Private Community" })).toBeTruthy();
          expect(screen.queryByText("Official updates.")).toBeNull();
          expect(screen.queryByRole("tab")).toBeNull();
          expect(net.fetchCommunityMembers).not.toHaveBeenCalled();
          expect(net.fetchCommunityEvents).not.toHaveBeenCalled();
        });
      });

      describe("and the link names one of its events", () => {
        beforeEach(() => {
          show(`/social/communities/${ID}?event=ev-1`);
        });

        it("should open no detail and read no event", async () => {
          expect(await screen.findByRole("heading", { name: "This is a Private Community" })).toBeTruthy();
          await act(async () => {});
          expect(screen.queryByRole("dialog")).toBeNull();
          expect(events.fetchEvent).not.toHaveBeenCalled();
        });
      });

      describe("and the browser already read one of its events elsewhere", () => {
        let router: Router;

        beforeEach(() => {
          const hosted = { ...upcomingEvent(), community_id: ID, user: "0x4444444444444444444444444444444444444444" };
          router = show(`/social/communities/${ID}?event=ev-1`, SID, (seeded) => {
            seeded.setQueryData(eventsPageKeys.detail("ev-1"), hosted);
            seeded.setQueryData(communityKeys.events(ID), {
              pages: [{ events: [hosted], total: 1, received: 1 }],
              pageParams: [0],
            });
          });
        });

        it("should open no detail and keep the link", async () => {
          expect(await screen.findByRole("heading", { name: "This is a Private Community" })).toBeTruthy();
          await act(async () => {});
          expect(screen.queryByRole("dialog")).toBeNull();
          expect(router.state.location.search).toBe("?event=ev-1");
          expect(creators.asked).not.toContain("0x4444444444444444444444444444444444444444");
        });
      });

      describe("and asking to join succeeds", () => {
        let request: { id: string; communityId: string; type: string; status: string };

        beforeEach(() => {
          request = { id: "r1", communityId: ID, type: "request_to_join", status: "pending" };
          net.requestToJoin.mockResolvedValue(request);
          net.fetchJoinRequests.mockResolvedValueOnce([]).mockResolvedValue([request]);
          show();
        });

        it("should send the request and offer to cancel it", async () => {
          await userEvent.click(await screen.findByRole("button", { name: "Request to join" }));

          expect(await screen.findByRole("button", { name: "Cancel request" })).toBeTruthy();
          expect(net.requestToJoin).toHaveBeenCalledWith(auth.identity, ID);
        });
      });

      describe("and asking to join fails", () => {
        beforeEach(() => {
          net.requestToJoin.mockRejectedValue(new Error("500"));
          show();
        });

        it("should keep the request button and the member's requests", async () => {
          await userEvent.click(await screen.findByRole("button", { name: "Request to join" }));

          expect((await screen.findByRole("alert")).textContent).toBe("Failed to send your request to join");
          expect(screen.getByRole("button", { name: "Request to join" })).toHaveProperty("disabled", false);
          expect(client.getQueryData(["private", ME, "community", "requests"])).toEqual([]);
        });
      });

      describe("and a request is pending", () => {
        let request: { id: string; communityId: string; type: string; status: string };

        beforeEach(() => {
          request = { id: "r1", communityId: ID, type: "request_to_join", status: "pending" };
        });

        describe("and cancelling it fails", () => {
          beforeEach(() => {
            net.fetchJoinRequests.mockResolvedValue([request]);
            net.cancelJoinRequest.mockRejectedValue(new Error("500"));
            show();
          });

          it("should keep the pending request", async () => {
            await userEvent.click(await screen.findByRole("button", { name: "Cancel request" }));

            expect((await screen.findByRole("alert")).textContent).toBe("Failed to cancel your request");
            expect(screen.getByRole("button", { name: "Cancel request" })).toHaveProperty("disabled", false);
            expect(net.cancelJoinRequest).toHaveBeenCalledWith(auth.identity, ID, "r1");
            expect(client.getQueryData(["private", ME, "community", "requests"])).toEqual([
              { id: "r1", communityId: ID, type: "request_to_join", status: "pending" },
            ]);
          });
        });

        describe("and cancelling it succeeds", () => {
          beforeEach(() => {
            net.fetchJoinRequests.mockResolvedValueOnce([request]).mockResolvedValue([]);
            net.cancelJoinRequest.mockResolvedValue(undefined);
            show();
          });

          it("should cancel the request and offer to request again", async () => {
            await userEvent.click(await screen.findByRole("button", { name: "Cancel request" }));

            expect(await screen.findByRole("button", { name: "Request to join" })).toBeTruthy();
          });
        });
      });

      describe("and the requests read fails", () => {
        beforeEach(() => {
          net.fetchJoinRequests.mockRejectedValue(new Error("502"));
          show();
        });

        it("should offer Try again instead of the request button", async () => {
          expect(await screen.findByRole("button", { name: "Try again" })).toBeTruthy();
          expect(screen.queryByRole("button", { name: "Request to join" })).toBeNull();
        });
      });

      describe("and the link is this tab's sign-in return with a request to join", () => {
        beforeEach(() => {
          net.requestToJoin.mockResolvedValue({ id: "r1", communityId: ID, type: "request_to_join", status: "pending" });
          const nonce = rememberAction(ID, "requestToJoin");
          show(`/social/communities/${ID}?action=requestToJoin&nonce=${nonce}`);
        });

        it("should run the request to join", async () => {
          await waitFor(() => expect(net.requestToJoin).toHaveBeenCalledTimes(1));
          expect(net.joinCommunity).not.toHaveBeenCalled();
        });
      });
    });

    describe("and the visitor is a member of a private community", () => {
      beforeEach(() => {
        serve(community({ privacy: "private" }), { role: "member" });
        show();
      });

      it("should show the members, read signed", async () => {
        expect(await screen.findByText("Alice")).toBeTruthy();
        expect(screen.getByRole("button", { name: "Joined" })).toHaveProperty("disabled", true);
        expect(net.fetchCommunityMembers).toHaveBeenCalledWith(ID, 0, expect.objectContaining({ identity: auth.identity }));
      });
    });

    describe("and the member's own read fails", () => {
      let anonymous: CommunityInfo;

      beforeEach(() => {
        anonymous = community();
        net.fetchCommunity.mockImplementation(async (_id: string, opts: { identity?: unknown } = {}) => {
          if (opts.identity) throw new Error("401");
          return anonymous;
        });
        show();
      });

      it("should offer Try again and recover", async () => {
        await userEvent.click(await screen.findByRole("button", { name: "Try again" }));
        expect(screen.getByRole("alert").textContent).toBe("We couldn't check your membership right now.");

        net.fetchCommunity.mockImplementation(async (_id: string, opts: { identity?: unknown } = {}) =>
          opts.identity ? { ...anonymous, role: "none" } : anonymous,
        );
        await userEvent.click(screen.getByRole("button", { name: "Try again" }));
        expect(await screen.findByRole("button", { name: "Join" })).toBeTruthy();
      });
    });

    describe("and the member's own read of a private community is still running", () => {
      let anonymous: CommunityInfo;
      let release: (value: CommunityInfo) => void;

      beforeEach(() => {
        anonymous = community({ privacy: "private" });
        release = () => {};
        net.fetchCommunity.mockImplementation((_id: string, opts: { identity?: unknown } = {}) =>
          opts.identity ? new Promise<CommunityInfo>((resolve) => (release = resolve)) : Promise.resolve(anonymous),
        );
        show();
      });

      it("should not flash the private notice to a member", async () => {
        expect(await screen.findByRole("progressbar", { name: "Loading community" })).toBeTruthy();
        expect(screen.queryByRole("heading", { name: "This is a Private Community" })).toBeNull();

        release({ ...anonymous, role: "member" });
        expect(await screen.findByText("Alice")).toBeTruthy();
        expect(screen.queryByRole("heading", { name: "This is a Private Community" })).toBeNull();
      });
    });
  });
});

describe("when a signed-out visitor asks for an event reminder", () => {
  it("sends a signed-out visitor to the sign-in page when a Remind me needs it", async () => {
    serve(community());
    show();

    await userEvent.click(await screen.findByRole("tab", { name: "Upcoming Events" }));
    await waitFor(() => expect(reminders.onSignIn).not.toBeNull());
    reminders.onSignIn!();

    // No options: the store sends the reader to /auth/login, to come back to this page (signin-store.test).
    expect(signIn.openSignIn).toHaveBeenCalledWith();
  });

});
