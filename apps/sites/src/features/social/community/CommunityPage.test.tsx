// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
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
const reminders = vi.hoisted(() => ({ failed: null as string | null }));

vi.mock("@/data/lib/catalyst/social/community-detail", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/social/community-detail")>()),
  ...net,
}));
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => ({ identity: auth.identity }) }));
vi.mock("../../components/auth/signin-store", () => signIn);
vi.mock("@ui/web/frames/SitesChrome", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));
vi.mock("../../events/useCreatorProfiles", () => ({ useCreatorProfiles: () => ({}) }));
vi.mock("../../events/useEventReminders", () => ({
  REMIND_FAILED: "Couldn't update your reminder. Try again.",
  useEventReminders: () => ({ failed: reminders.failed, get: () => ({ active: false, pending: false, toggle: () => {} }) }),
}));

import type { CommunityInfo } from "@/data/lib/catalyst/social/community-detail";

import { CommunityPage } from "./CommunityPage";
import { PENDING_ACTION_TTL_MS, rememberAction } from "./pendingAction";

const ID = "e99471aa-31c4-4952-abf6-99905445f43b";
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

let client: QueryClient;

function show(path = `/social/communities/${ID}`) {
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter([{ path: "/social/communities/:id", element: <CommunityPage id={ID} now={0} /> }], {
    initialEntries: [path],
  });
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
  signIn.openSignIn.mockReset();
  auth.identity = null;
  reminders.failed = null;
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
  vi.restoreAllMocks();
});

const redirectOf = () => new URL(signIn.openSignIn.mock.calls[0]![0].redirectTo, "http://localhost");

describe("CommunityPage", () => {
  it("shows a public community's card, members and events to a signed-out visitor", async () => {
    serve(community());
    show();

    expect(await screen.findByRole("heading", { level: 1, name: "Decentraland Foundation" })).toBeTruthy();
    expect(screen.getByText("1.3K Members")).toBeTruthy();
    expect(screen.getByText("Official updates.")).toBeTruthy();
    expect(await screen.findByText("Alice")).toBeTruthy();
    expect(screen.getByText("Owner")).toBeTruthy();
    expect(await screen.findByText("No Upcoming Events")).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Members", selected: true })).toBeTruthy();
  });

  it("sends a signed-out visitor to sign in and back with the join action", async () => {
    serve(community());
    show();

    await userEvent.click(await screen.findByRole("button", { name: "Sign in to join" }));

    const back = redirectOf();
    expect(back.pathname).toBe(`/social/communities/${ID}`);
    expect(back.searchParams.get("action")).toBe("join");
    expect(back.searchParams.get("nonce")).toBeTruthy();
  });

  it("asks a private community's sign-in to come back with a request to join", async () => {
    serve(community({ privacy: "private" }));
    show();

    await userEvent.click(await screen.findByRole("button", { name: "Sign in to join" }));

    expect(redirectOf().searchParams.get("action")).toBe("requestToJoin");
  });

  it("returns to the plain page when the tab cannot record the action", async () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("QuotaExceededError");
    });
    serve(community());
    show();

    await userEvent.click(await screen.findByRole("button", { name: "Sign in to join" }));

    expect(signIn.openSignIn).toHaveBeenCalledWith({ redirectTo: `/social/communities/${ID}` });
  });

  it("hides a private community's members, events and description from a non-member", async () => {
    auth.identity = { signer: ME };
    serve(community({ privacy: "private" }), { role: "none" });
    show();

    expect(await screen.findByRole("heading", { name: "This is a Private Community" })).toBeTruthy();
    expect(screen.queryByText("Official updates.")).toBeNull();
    expect(screen.queryByRole("tab")).toBeNull();
    expect(net.fetchCommunityMembers).not.toHaveBeenCalled();
    expect(net.fetchCommunityEvents).not.toHaveBeenCalled();
  });

  it("shows a private community's members to a member, read signed", async () => {
    auth.identity = { signer: ME };
    serve(community({ privacy: "private" }), { role: "member" });
    show();

    expect(await screen.findByText("Alice")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Joined" })).toHaveProperty("disabled", true);
    expect(net.fetchCommunityMembers).toHaveBeenCalledWith(ID, 0, expect.objectContaining({ identity: auth.identity }));
  });

  it("joins a public community and shows the member as joined", async () => {
    auth.identity = { signer: ME };
    let role: CommunityInfo["role"] = "none";
    net.fetchCommunity.mockImplementation(async (_id: string, opts: { identity?: unknown } = {}) =>
      community(opts.identity ? { role } : {}),
    );
    net.joinCommunity.mockImplementation(async () => {
      role = "member";
    });
    show();

    await userEvent.click(await screen.findByRole("button", { name: "Join" }));

    expect(await screen.findByRole("button", { name: "Joined" })).toBeTruthy();
    expect(net.joinCommunity).toHaveBeenCalledWith(auth.identity, ID);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("keeps the member out and says so when joining fails", async () => {
    auth.identity = { signer: ME };
    serve(community(), { role: "none" });
    net.joinCommunity.mockRejectedValue(new Error("500"));
    show();

    await userEvent.click(await screen.findByRole("button", { name: "Join" }));

    expect((await screen.findByRole("alert")).textContent).toBe("Failed to join community");
    expect(screen.getByRole("button", { name: "Join" })).toHaveProperty("disabled", false);
    expect(screen.queryByRole("button", { name: "Joined" })).toBeNull();
    // Nothing was re-read: the member's standing is what it was.
    expect(net.fetchCommunity).toHaveBeenCalledTimes(2);
  });

  it("asks to join a private community, then offers to cancel the request", async () => {
    auth.identity = { signer: ME };
    serve(community({ privacy: "private" }), { role: "none" });
    const request = { id: "r1", communityId: ID, type: "request_to_join", status: "pending" };
    net.requestToJoin.mockResolvedValue(request);
    net.fetchJoinRequests.mockResolvedValueOnce([]).mockResolvedValue([request]);
    show();

    await userEvent.click(await screen.findByRole("button", { name: "Request to join" }));

    expect(await screen.findByRole("button", { name: "Cancel request" })).toBeTruthy();
    expect(net.requestToJoin).toHaveBeenCalledWith(auth.identity, ID);
  });

  it("keeps a pending request when cancelling it fails", async () => {
    auth.identity = { signer: ME };
    serve(community({ privacy: "private" }), { role: "none" });
    net.fetchJoinRequests.mockResolvedValue([{ id: "r1", communityId: ID, type: "request_to_join", status: "pending" }]);
    net.cancelJoinRequest.mockRejectedValue(new Error("500"));
    show();

    await userEvent.click(await screen.findByRole("button", { name: "Cancel request" }));

    expect((await screen.findByRole("alert")).textContent).toBe("Failed to cancel your request");
    expect(screen.getByRole("button", { name: "Cancel request" })).toHaveProperty("disabled", false);
    expect(net.cancelJoinRequest).toHaveBeenCalledWith(auth.identity, ID, "r1");
    expect(client.getQueryData(["private", ME, "community", "requests"])).toEqual([
      { id: "r1", communityId: ID, type: "request_to_join", status: "pending" },
    ]);
  });

  it("cancels a pending request", async () => {
    auth.identity = { signer: ME };
    serve(community({ privacy: "private" }), { role: "none" });
    const request = { id: "r1", communityId: ID, type: "request_to_join", status: "pending" };
    net.fetchJoinRequests.mockResolvedValueOnce([request]).mockResolvedValue([]);
    net.cancelJoinRequest.mockResolvedValue(undefined);
    show();

    await userEvent.click(await screen.findByRole("button", { name: "Cancel request" }));

    expect(await screen.findByRole("button", { name: "Request to join" })).toBeTruthy();
  });

  it("runs the join this tab's sign-in came back with, once, and consumes its marker", async () => {
    auth.identity = { signer: ME };
    serve(community(), { role: "none" });
    net.joinCommunity.mockResolvedValue(undefined);
    const nonce = rememberAction(ID, "join");
    const router = show(`/social/communities/${ID}?action=join&nonce=${nonce}`);

    await waitFor(() => expect(net.joinCommunity).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(router.state.location.search).toBe(""));
    expect(net.joinCommunity).toHaveBeenCalledTimes(1);
    expect(sessionStorage.length).toBe(0);
  });

  it("does not join from a crafted link this tab did not start", async () => {
    auth.identity = { signer: ME };
    serve(community(), { role: "none" });
    const router = show(`/social/communities/${ID}?action=join&nonce=guessed`);

    await waitFor(() => expect(router.state.location.search).toBe(""));
    expect(net.joinCommunity).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Join" })).toBeTruthy();
  });

  it("does not join from a link without a nonce", async () => {
    auth.identity = { signer: ME };
    serve(community(), { role: "none" });
    rememberAction(ID, "join");
    const router = show(`/social/communities/${ID}?action=join`);

    await waitFor(() => expect(router.state.location.search).toBe(""));
    expect(net.joinCommunity).not.toHaveBeenCalled();
  });

  it("does not join on an expired marker", async () => {
    auth.identity = { signer: ME };
    serve(community(), { role: "none" });
    const nonce = rememberAction(ID, "join", sessionStorage, Date.now() - PENDING_ACTION_TTL_MS - 1);
    const router = show(`/social/communities/${ID}?action=join&nonce=${nonce}`);

    await waitFor(() => expect(router.state.location.search).toBe(""));
    expect(net.joinCommunity).not.toHaveBeenCalled();
  });

  it("does not join on a marker for another community", async () => {
    auth.identity = { signer: ME };
    serve(community(), { role: "none" });
    const nonce = rememberAction("00000000-0000-0000-0000-000000000000", "join");
    const router = show(`/social/communities/${ID}?action=join&nonce=${nonce}`);

    await waitFor(() => expect(router.state.location.search).toBe(""));
    expect(net.joinCommunity).not.toHaveBeenCalled();
  });

  it("does not join when the tab's storage cannot be read", async () => {
    auth.identity = { signer: ME };
    serve(community(), { role: "none" });
    const nonce = rememberAction(ID, "join");
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("SecurityError");
    });
    const router = show(`/social/communities/${ID}?action=join&nonce=${nonce}`);

    await waitFor(() => expect(router.state.location.search).toBe(""));
    expect(net.joinCommunity).not.toHaveBeenCalled();
  });

  it("drops an action that no longer applies without running it", async () => {
    auth.identity = { signer: ME };
    serve(community(), { role: "member" });
    const nonce = rememberAction(ID, "join");
    const router = show(`/social/communities/${ID}?action=join&nonce=${nonce}`);

    await waitFor(() => expect(router.state.location.search).toBe(""));
    expect(net.joinCommunity).not.toHaveBeenCalled();
    expect(sessionStorage.length).toBe(0);
  });

  it("drops an unknown action", async () => {
    serve(community());
    const router = show(`/social/communities/${ID}?action=leave`);

    await waitFor(() => expect(router.state.location.search).toBe(""));
  });

  it("keeps the request button and the member's requests when asking to join fails", async () => {
    auth.identity = { signer: ME };
    serve(community({ privacy: "private" }), { role: "none" });
    net.requestToJoin.mockRejectedValue(new Error("500"));
    show();

    await userEvent.click(await screen.findByRole("button", { name: "Request to join" }));

    expect((await screen.findByRole("alert")).textContent).toBe("Failed to send your request to join");
    expect(screen.getByRole("button", { name: "Request to join" })).toHaveProperty("disabled", false);
    expect(client.getQueryData(["private", ME, "community", "requests"])).toEqual([]);
  });

  it("offers Try again when the member's own read fails, and recovers", async () => {
    auth.identity = { signer: ME };
    const anonymous = community();
    net.fetchCommunity.mockImplementation(async (_id: string, opts: { identity?: unknown } = {}) => {
      if (opts.identity) throw new Error("401");
      return anonymous;
    });
    show();

    await userEvent.click(await screen.findByRole("button", { name: "Try again" }));
    expect(screen.getByRole("alert").textContent).toBe("We couldn't check your membership right now.");

    net.fetchCommunity.mockImplementation(async (_id: string, opts: { identity?: unknown } = {}) =>
      opts.identity ? { ...anonymous, role: "none" } : anonymous,
    );
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("button", { name: "Join" })).toBeTruthy();
  });

  it("offers Try again when a private community's requests read fails", async () => {
    auth.identity = { signer: ME };
    serve(community({ privacy: "private" }), { role: "none" });
    net.fetchJoinRequests.mockRejectedValue(new Error("502"));
    show();

    expect(await screen.findByRole("button", { name: "Try again" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Request to join" })).toBeNull();
  });

  it("does not flash the private notice to a member while their read runs", async () => {
    auth.identity = { signer: ME };
    const anonymous = community({ privacy: "private" });
    let release: (value: CommunityInfo) => void = () => {};
    net.fetchCommunity.mockImplementation((_id: string, opts: { identity?: unknown } = {}) =>
      opts.identity ? new Promise<CommunityInfo>((resolve) => (release = resolve)) : Promise.resolve(anonymous),
    );
    show();

    expect(await screen.findByRole("progressbar", { name: "Loading community" })).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "This is a Private Community" })).toBeNull();

    release({ ...anonymous, role: "member" });
    expect(await screen.findByText("Alice")).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "This is a Private Community" })).toBeNull();
  });

  it("runs a request to join this tab's sign-in came back with", async () => {
    auth.identity = { signer: ME };
    serve(community({ privacy: "private" }), { role: "none" });
    net.requestToJoin.mockResolvedValue({ id: "r1", communityId: ID, type: "request_to_join", status: "pending" });
    const nonce = rememberAction(ID, "requestToJoin");
    show(`/social/communities/${ID}?action=requestToJoin&nonce=${nonce}`);

    await waitFor(() => expect(net.requestToJoin).toHaveBeenCalledTimes(1));
    expect(net.joinCommunity).not.toHaveBeenCalled();
  });

  it("keeps a sign-in return's action while the visitor is still signed out", async () => {
    serve(community());
    const nonce = rememberAction(ID, "join");
    const router = show(`/social/communities/${ID}?action=join&nonce=${nonce}`);

    expect(await screen.findByRole("button", { name: "Sign in to join" })).toBeTruthy();
    expect(router.state.location.search).toBe(`?action=join&nonce=${nonce}`);
    expect(sessionStorage.length).toBe(1);
  });

  it("says so inline when the members cannot be read, keeping the card", async () => {
    serve(community());
    net.fetchCommunityMembers.mockRejectedValue(new Error("502"));
    show();

    expect(await screen.findByText("We couldn't load the members right now.")).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1, name: "Decentraland Foundation" })).toBeTruthy();
  });

  it("asks once and offers Try again when the next members page fails, keeping the first", async () => {
    // The sentinel is always on screen here, as on a short list.
    class Visible {
      constructor(private readonly cb: IntersectionObserverCallback) {}
      observe() {
        this.cb([{ isIntersecting: true } as IntersectionObserverEntry], this as unknown as IntersectionObserver);
      }
      disconnect() {}
    }
    vi.stubGlobal("IntersectionObserver", Visible);
    serve(community());
    const first = { memberAddress: ME, role: "owner", name: "Alice", profilePictureUrl: null, hasClaimedName: true };
    net.fetchCommunityMembers.mockImplementation(async (_id: string, offset: number) => {
      if (offset === 0) return { members: [first], total: 2, hasMore: true };
      throw new Error("502");
    });
    show();

    expect(await screen.findByText("We couldn't load more members.")).toBeTruthy();
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(net.fetchCommunityMembers).toHaveBeenCalledTimes(2);
    expect(screen.getByText("Alice")).toBeTruthy();

    net.fetchCommunityMembers.mockImplementation(async (_id: string, offset: number) =>
      offset === 0
        ? { members: [first], total: 2, hasMore: true }
        : { members: [{ ...first, memberAddress: "0x2222222222222222222222222222222222222222", name: "Bob", role: "member" }], total: 2, hasMore: false },
    );
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("Bob")).toBeTruthy();
    vi.unstubAllGlobals();
  });

  it("keeps the members in their own scroller and watches its end, not the page's, for the next page", async () => {
    const roots: (Element | Document | null | undefined)[] = [];
    class Recorded {
      constructor(_cb: IntersectionObserverCallback, options?: IntersectionObserverInit) {
        roots.push(options?.root);
      }
      observe() {}
      disconnect() {}
    }
    vi.stubGlobal("IntersectionObserver", Recorded);
    serve(community());
    const first = { memberAddress: ME, role: "owner", name: "Alice", profilePictureUrl: null, hasClaimedName: true };
    net.fetchCommunityMembers.mockResolvedValue({ members: [first], total: 2, hasMore: true });
    show();

    const list = await screen.findByRole("group", { name: "Members (2)" });
    expect(list.tabIndex).toBe(0);
    expect(within(list).getByText("Alice")).toBeTruthy();
    await waitFor(() => expect(roots).toContain(list));
    vi.unstubAllGlobals();
  });

  it("shows no members scroller when the community has no members", async () => {
    serve(community());
    net.fetchCommunityMembers.mockResolvedValue({ members: [], total: 0, hasMore: false });
    show();

    expect(await screen.findByText("No members found")).toBeTruthy();
    expect(screen.queryByRole("group", { name: /Members/ })).toBeNull();
  });

  it("offers Try again when the first members page fails, and shows them once it loads", async () => {
    serve(community());
    net.fetchCommunityMembers.mockRejectedValueOnce(new Error("502"));
    show();

    expect(await screen.findByText("We couldn't load the members right now.")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("Alice")).toBeTruthy();
  });

  it("offers Try again when the first events page fails", async () => {
    serve(community());
    net.fetchCommunityEvents.mockRejectedValueOnce(new Error("502"));
    show();

    expect(await screen.findByText("We couldn't load the events right now.")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("No Upcoming Events")).toBeTruthy();
  });

  it("offers Load more instead of loading on scroll when the browser has no IntersectionObserver", async () => {
    vi.stubGlobal("IntersectionObserver", undefined);
    serve(community());
    const first = { memberAddress: ME, role: "owner", name: "Alice", profilePictureUrl: null, hasClaimedName: true };
    net.fetchCommunityMembers.mockImplementation(async (_id: string, offset: number) =>
      offset === 0
        ? { members: [first], total: 2, hasMore: true }
        : { members: [{ ...first, memberAddress: "0x2222222222222222222222222222222222222222", name: "Bob", role: "member" }], total: 2, hasMore: false },
    );
    show();

    await userEvent.click(await screen.findByRole("button", { name: "Load more" }));
    expect(await screen.findByText("Bob")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Load more" })).toBeNull();
    vi.unstubAllGlobals();
  });

  it("drops a failed join's message once another account is signed in", async () => {
    auth.identity = { signer: ME };
    serve(community(), { role: "none" });
    net.joinCommunity.mockRejectedValue(new Error("500"));
    const router = show();

    await userEvent.click(await screen.findByRole("button", { name: "Join" }));
    expect((await screen.findByRole("alert")).textContent).toBe("Failed to join community");

    auth.identity = { signer: "0x3333333333333333333333333333333333333333" };
    await router.navigate(`/social/communities/${ID}?again=1`);

    await waitFor(() => expect(screen.queryByText("Failed to join community")).toBeNull());
  });

  it("says so when a Remind me toggle fails", async () => {
    serve(community());
    reminders.failed = "e1";
    show();

    expect(await screen.findByText("Couldn't update your reminder. Try again.")).toBeTruthy();
  });

  it("switches between members and events with the tabs", async () => {
    serve(community());
    show();

    await userEvent.click(await screen.findByRole("tab", { name: "Upcoming Events" }));

    expect(screen.getByRole("tab", { name: "Upcoming Events", selected: true })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Members", selected: false })).toBeTruthy();
  });
});
