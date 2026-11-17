// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
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

vi.mock("@/data/lib/catalyst/social/community-detail", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/social/community-detail")>()),
  ...net,
}));
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => ({ identity: auth.identity }) }));
vi.mock("../../components/auth/signin-store", () => signIn);
vi.mock("@ui/web/frames/SitesChrome", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));
vi.mock("../../events/useCreatorProfiles", () => ({ useCreatorProfiles: () => ({}) }));
vi.mock("../../events/useEventReminders", () => ({
  useEventReminders: () => ({ get: () => ({ active: false, pending: false, toggle: () => {} }) }),
}));

import type { Community } from "@/data/lib/catalyst/social/community-detail";

import { CommunityPage } from "./CommunityPage";

const ID = "e99471aa-31c4-4952-abf6-99905445f43b";
const ME = "0x1111111111111111111111111111111111111111";

const community = (over: Partial<Community> = {}): Community => ({
  id: ID,
  name: "Decentraland Foundation",
  description: "Official updates.",
  ownerAddress: "0xbcac4dafb7e215f2f6cb3312af6d5e4f9d9e7eda",
  ownerName: "DCLOfficial",
  privacy: "public",
  membersCount: 1340,
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
function serve(anonymous: Community, viewer?: Partial<Community>) {
  net.fetchCommunity.mockImplementation(async (_id: string, opts: { identity?: unknown } = {}) =>
    opts.identity ? { ...anonymous, ...viewer } : anonymous,
  );
}

beforeEach(() => {
  for (const fn of Object.values(net)) fn.mockReset();
  signIn.openSignIn.mockReset();
  auth.identity = null;
  net.fetchCommunityMembers.mockResolvedValue({
    members: [{ memberAddress: ME, role: "owner", name: "Alice", profilePictureUrl: null, hasClaimedName: true }],
    total: 1,
    hasMore: false,
  });
  net.fetchCommunityEvents.mockResolvedValue({ events: [], total: 0 });
  net.fetchJoinRequests.mockResolvedValue([]);
});
afterEach(() => cleanup());

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

    expect(signIn.openSignIn).toHaveBeenCalledWith({ redirectTo: `/social/communities/${ID}?action=join` });
  });

  it("asks a private community's sign-in to come back with a request to join", async () => {
    serve(community({ privacy: "private" }));
    show();

    await userEvent.click(await screen.findByRole("button", { name: "Sign in to join" }));

    expect(signIn.openSignIn).toHaveBeenCalledWith({ redirectTo: `/social/communities/${ID}?action=requestToJoin` });
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
    let role: Community["role"] = "none";
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

    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Cancel request" })).toHaveProperty("disabled", false);
    expect(net.cancelJoinRequest).toHaveBeenCalledWith(auth.identity, ID, "r1");
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

  it("runs the join a sign-in came back with, once, and drops it from the URL", async () => {
    auth.identity = { signer: ME };
    serve(community(), { role: "none" });
    net.joinCommunity.mockResolvedValue(undefined);
    const router = show(`/social/communities/${ID}?action=join`);

    await waitFor(() => expect(net.joinCommunity).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(router.state.location.search).toBe(""));
    expect(net.joinCommunity).toHaveBeenCalledTimes(1);
  });

  it("drops an action that no longer applies without running it", async () => {
    auth.identity = { signer: ME };
    serve(community(), { role: "member" });
    const router = show(`/social/communities/${ID}?action=join`);

    await waitFor(() => expect(router.state.location.search).toBe(""));
    expect(net.joinCommunity).not.toHaveBeenCalled();
  });

  it("drops an unknown action", async () => {
    serve(community());
    const router = show(`/social/communities/${ID}?action=leave`);

    await waitFor(() => expect(router.state.location.search).toBe(""));
  });

  it("switches between members and events with the tabs", async () => {
    serve(community());
    show();

    await userEvent.click(await screen.findByRole("tab", { name: "Upcoming Events" }));

    expect(screen.getByRole("tab", { name: "Upcoming Events", selected: true })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Members", selected: false })).toBeTruthy();
  });
});
