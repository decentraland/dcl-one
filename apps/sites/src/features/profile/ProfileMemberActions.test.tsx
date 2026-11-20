// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
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
const net = vi.hoisted(() => ({
  fetchFriendship: vi.fn(),
  fetchFriendsCount: vi.fn(),
  fetchMutualFriendsCount: vi.fn(),
  fetchBlocking: vi.fn(),
  changeFriendship: vi.fn(),
  setBlocked: vi.fn(),
  openSignIn: vi.fn(),
}));
const auth = vi.hoisted(() => ({ state: { identity: null as unknown, isConnected: false, address: null as string | null } }));
vi.mock("@/data/lib/friends/friends", () => ({
  fetchFriendship: net.fetchFriendship,
  fetchFriendsCount: net.fetchFriendsCount,
  fetchMutualFriendsCount: net.fetchMutualFriendsCount,
  fetchBlocking: net.fetchBlocking,
  changeFriendship: net.changeFriendship,
  setBlocked: net.setBlocked,
}));
vi.mock("@/data/lib/auth/index", () => ({ useAuth: () => auth.state }));
vi.mock("../components/auth/signin-store", () => ({ openSignIn: net.openSignIn }));
vi.mock("@/data/lib/catalyst/social/badges", () => ({ fetchUserBadges: () => Promise.resolve([]) }));

import { emptyProfile } from "@/data/lib/catalyst/social/profile";
import { friendsKeys } from "@/data/lib/friends/queries";

import { ProfileFriendsCount, ProfileMemberActions } from "./ProfileMemberActions";
import { ProfileShell } from "./ProfileShell";

const ME = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const OTHER = "0x1111111111111111111111111111111111111111";
const identity = { signer: ME, ephemeral: { address: "0xeph", privateKey: "0xdeadbeef" }, expiration: "2999-01-01T00:00:00.000Z", authChain: [] };

let client: QueryClient;

function show(ui = <ProfileMemberActions address={OTHER} name="Bob" />) {
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(<QueryClientProvider client={client}>{ui}</QueryClientProvider>);
}

const button = (name: string) => screen.getByRole("button", { name });
const findButton = (name: string) => screen.findByRole("button", { name });

beforeEach(() => {
  auth.state = { identity, isConnected: true, address: ME };
  net.fetchFriendship.mockResolvedValue("none");
  net.fetchBlocking.mockResolvedValue({ blocked: [], blockedBy: [] });
  net.fetchMutualFriendsCount.mockResolvedValue(0);
  net.fetchFriendsCount.mockResolvedValue(17);
  net.changeFriendship.mockResolvedValue(undefined);
  net.setBlocked.mockResolvedValue(undefined);
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  vi.unstubAllGlobals();
});

describe("a member's header actions, signed out", () => {
  beforeEach(() => {
    auth.state = { identity: null, isConnected: false, address: null };
  });

  it("opens sign-in from Add friend, offers no block, and reads nothing", async () => {
    const user = userEvent.setup();
    show();

    await user.click(button("Add friend"));

    expect(net.openSignIn).toHaveBeenCalledWith({ inPlace: true });
    expect(screen.queryByRole("button", { name: "Block" })).toBeNull();
    expect(net.fetchFriendship).not.toHaveBeenCalled();
    expect(net.fetchBlocking).not.toHaveBeenCalled();
    expect(net.changeFriendship).not.toHaveBeenCalled();
  });

  it("copies the profile's link from Share and says so", async () => {
    const user = userEvent.setup();
    const writeText = vi.fn(async () => {});
    vi.stubGlobal("navigator", { ...navigator, clipboard: { writeText } });
    show();

    await user.click(button("Share"));

    expect(writeText).toHaveBeenCalledWith(`${window.location.origin}/profile/${OTHER}`);
    expect(await screen.findByText("Profile link copied")).toBeTruthy();
  });
});

describe("a member's header actions, friendship", () => {
  it("shows the friendship as loading until it is read, signed for the member", async () => {
    let answer: (value: string) => void = () => {};
    net.fetchFriendship.mockReturnValue(new Promise<string>((resolve) => (answer = resolve)));
    show();

    // A loading Button keeps the focus and refuses the press.
    expect(button("Loading friendship").getAttribute("aria-disabled")).toBe("true");
    answer("none");

    expect((await findButton("Add friend")).getAttribute("aria-disabled")).not.toBe("true");
    expect(net.fetchFriendship.mock.calls[0]?.slice(0, 2)).toEqual([identity, OTHER]);
  });

  it("sends a request from Add friend, then shows it as sent with the way to cancel it", async () => {
    const user = userEvent.setup();
    show();
    const add = await findButton("Add friend");
    net.fetchFriendship.mockResolvedValue("request_sent");

    await user.click(add);

    expect(net.changeFriendship).toHaveBeenCalledWith(identity, OTHER, "request");
    expect(await screen.findByText("Friend request sent.")).toBeTruthy();
    expect(await screen.findByText("Request sent")).toBeTruthy();
    expect(button("Cancel request")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Add friend" })).toBeNull();
  });

  it("cancels a request the member sent", async () => {
    const user = userEvent.setup();
    net.fetchFriendship.mockResolvedValue("request_sent");
    show();
    const cancel = await findButton("Cancel request");
    net.fetchFriendship.mockResolvedValue("none");

    await user.click(cancel);

    expect(net.changeFriendship).toHaveBeenCalledWith(identity, OTHER, "cancel");
    expect(await findButton("Add friend")).toBeTruthy();
    expect(screen.getByText("Friend request cancelled.")).toBeTruthy();
  });

  it("accepts a request the member received, and then shows Friends with Unfriend", async () => {
    const user = userEvent.setup();
    net.fetchFriendship.mockResolvedValue("request_received");
    show();
    const accept = await findButton("Accept friend request");
    expect(button("Reject friend request")).toBeTruthy();
    net.fetchFriendship.mockResolvedValue("friends");

    await user.click(accept);

    expect(net.changeFriendship).toHaveBeenCalledWith(identity, OTHER, "accept");
    expect(await findButton("Unfriend")).toBeTruthy();
    expect(screen.getByText("Friends")).toBeTruthy();
    expect(screen.getByText("You and Bob are now friends.")).toBeTruthy();
  });

  it("rejects a request the member received", async () => {
    const user = userEvent.setup();
    net.fetchFriendship.mockResolvedValue("request_received");
    show();

    await user.click(await findButton("Reject friend request"));

    expect(net.changeFriendship).toHaveBeenCalledWith(identity, OTHER, "reject");
  });

  it("removes a friend from Unfriend", async () => {
    const user = userEvent.setup();
    net.fetchFriendship.mockResolvedValue("friends");
    show();
    const unfriend = await findButton("Unfriend");
    net.fetchFriendship.mockResolvedValue("none");

    await user.click(unfriend);

    expect(net.changeFriendship).toHaveBeenCalledWith(identity, OTHER, "remove");
    expect(await screen.findByText("Bob is no longer your friend.")).toBeTruthy();
    expect(await findButton("Add friend")).toBeTruthy();
  });

  it("says when a write fails, and reads the friendship again", async () => {
    const user = userEvent.setup();
    show();
    const add = await findButton("Add friend");
    net.changeFriendship.mockRejectedValue(new Error("409"));
    net.fetchFriendship.mockResolvedValue("request_received");

    await user.click(add);

    expect((await screen.findByRole("alert")).textContent).toBe("The friendship could not be updated. Try again.");
    // The other member had sent a request first: the page shows it now.
    expect(await findButton("Accept friend request")).toBeTruthy();
  });

  it("says when the friendship cannot be read, never Add friend, and reads again on Try again", async () => {
    const user = userEvent.setup();
    net.fetchFriendship.mockRejectedValueOnce(new Error("502"));
    show();

    expect(await screen.findByText("Friendship unavailable")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Add friend" })).toBeNull();
    await user.click(button("Try again"));

    expect(await findButton("Add friend")).toBeTruthy();
  });
});

describe("a member's header actions, block", () => {
  it("blocks the member, then shows them as blocked with Unblock and no friend action", async () => {
    const user = userEvent.setup();
    show();
    await findButton("Add friend");
    const block = await findButton("Block");
    await waitFor(() => expect(block).toHaveProperty("disabled", false));
    net.fetchBlocking.mockResolvedValue({ blocked: [OTHER], blockedBy: [] });

    await user.click(block);

    expect(net.setBlocked).toHaveBeenCalledWith(identity, OTHER, true);
    expect(await findButton("Unblock")).toBeTruthy();
    expect(screen.getByText("Blocked")).toBeTruthy();
    expect(screen.getByText("Bob has been blocked.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Add friend" })).toBeNull();
  });

  it("unblocks a blocked member, and the friend action comes back", async () => {
    const user = userEvent.setup();
    net.fetchBlocking.mockResolvedValue({ blocked: [OTHER], blockedBy: [] });
    show();
    const unblock = await findButton("Unblock");
    net.fetchBlocking.mockResolvedValue({ blocked: [], blockedBy: [] });

    await user.click(unblock);

    expect(net.setBlocked).toHaveBeenCalledWith(identity, OTHER, false);
    expect(await findButton("Add friend")).toBeTruthy();
    expect(screen.getByText("Bob has been unblocked.")).toBeTruthy();
  });

  it("offers no friend action to a visitor the member blocked", async () => {
    net.fetchBlocking.mockResolvedValue({ blocked: [], blockedBy: [OTHER] });
    show();

    await waitFor(() => expect(button("Block")).toHaveProperty("disabled", false));
    expect(screen.queryByRole("button", { name: "Add friend" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Loading friendship" })).toBeNull();
  });

  it("says when the block fails", async () => {
    const user = userEvent.setup();
    net.setBlocked.mockRejectedValue(new Error("500"));
    show();
    const block = await findButton("Block");
    await waitFor(() => expect(block).toHaveProperty("disabled", false));

    await user.click(block);

    expect((await screen.findByRole("alert")).textContent).toBe("Bob could not be blocked. Try again.");
    expect(button("Block")).toBeTruthy();
  });

  it("says when the block lists cannot be read, with no dead Block button, and reads again on Try again", async () => {
    const user = userEvent.setup();
    net.fetchBlocking.mockRejectedValueOnce(new Error("502"));
    show();

    expect(await screen.findByText("Block unavailable")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Block" })).toBeNull();
    // The friendship was read: its action stays.
    expect(await findButton("Add friend")).toBeTruthy();
    await user.click(button("Try again"));

    await waitFor(() => expect(button("Block")).toHaveProperty("disabled", false));
    expect(screen.queryByText("Block unavailable")).toBeNull();
    expect(net.fetchFriendship).toHaveBeenCalledTimes(1);
  });

  it("reads both again from one Try again when neither the friendship nor the block lists can be read", async () => {
    const user = userEvent.setup();
    net.fetchFriendship.mockRejectedValueOnce(new Error("502"));
    net.fetchBlocking.mockRejectedValueOnce(new Error("502"));
    show();

    expect(await screen.findByText("Friendship unavailable")).toBeTruthy();
    await waitFor(() => expect(screen.queryByRole("button", { name: "Block" })).toBeNull());
    expect(screen.getAllByRole("button", { name: "Try again" })).toHaveLength(1);
    await user.click(button("Try again"));

    expect(await findButton("Add friend")).toBeTruthy();
    await waitFor(() => expect(button("Block")).toHaveProperty("disabled", false));
  });
});

describe("friends counters", () => {
  it("shows the friends the visitor shares with the member, and nothing when there are none", async () => {
    net.fetchMutualFriendsCount.mockResolvedValue(3);
    show();

    expect(await screen.findByText("3 Mutual")).toBeTruthy();
    expect(client.getQueryData(friendsKeys.mutualCount(ME, OTHER))).toBe(3);

    cleanup();
    net.fetchMutualFriendsCount.mockResolvedValue(0);
    show();
    await findButton("Add friend");
    expect(screen.queryByText(/Mutual/)).toBeNull();
  });

  it.each([
    ["the visitor blocked the member", { blocked: [OTHER], blockedBy: [] }],
    ["the member blocked the visitor", { blocked: [], blockedBy: [OTHER] }],
  ])("shows no shared friends while %s", async (_case, lists) => {
    net.fetchMutualFriendsCount.mockResolvedValue(3);
    net.fetchBlocking.mockResolvedValue(lists);
    show();

    await waitFor(() => expect(client.getQueryData(friendsKeys.mutualCount(ME, OTHER))).toBe(3));
    await waitFor(() => expect(client.getQueryData(friendsKeys.blocking(ME))).toEqual(lists));
    expect(screen.queryByText(/Mutual/)).toBeNull();
  });

  it("shows the member's own friends count on their profile, read with their identity", async () => {
    show(<ProfileFriendsCount address={ME} />);

    expect(await screen.findByText("17 Friends")).toBeTruthy();
    expect(net.fetchFriendsCount.mock.calls[0]?.[0]).toBe(identity);
  });

  it("reads no friends count with another wallet's identity", () => {
    show(<ProfileFriendsCount address={OTHER} />);

    expect(net.fetchFriendsCount).not.toHaveBeenCalled();
  });
});

describe("the profile header", () => {
  function shell(own: boolean) {
    client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const address = own ? ME : OTHER;
    const profile = { ...emptyProfile(address), name: own ? "Alice" : "Bob" };
    const tabs = [{ id: "overview", label: "Overview", href: `/profile/${address}` }];
    const router = createMemoryRouter(
      [{ path: "*", element: <ProfileShell profile={profile} own={own} tabs={tabs} active="overview" onTab={vi.fn()}>body</ProfileShell> }],
      { initialEntries: [`/profile/${address}`] },
    );
    render(
      <QueryClientProvider client={client}>
        <RouterProvider router={router} />
      </QueryClientProvider>,
    );
  }

  it("gives another member's profile the friendship, block and share actions", async () => {
    shell(false);

    expect(await findButton("Add friend")).toBeTruthy();
    expect(button("Block")).toBeTruthy();
    expect(button("Share")).toBeTruthy();
    expect(screen.queryByRole("link", { name: "Get a Unique Name" })).toBeNull();
  });

  it("keeps the owner's call to action on their own profile, with their friends count and no friend actions", async () => {
    shell(true);

    expect(await screen.findByText("17 Friends")).toBeTruthy();
    expect(screen.getByRole("link", { name: "Get a Unique Name" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Add friend" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Block" })).toBeNull();
    expect(net.fetchFriendship).not.toHaveBeenCalled();
  });
});
