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
  fetchFriendsPage: vi.fn(),
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
  fetchFriendsPage: net.fetchFriendsPage,
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

import { ProfileMemberActions } from "./ProfileMemberActions";
import { ProfileFriendsCount, ProfileInviteFriends } from "./ProfileOwnerActions";
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
const item = (name: string) => screen.getByRole("menuitem", { name });
type User = ReturnType<typeof userEvent.setup>;
async function openMenu(user: User) {
  const more = button("More actions");
  if (more.getAttribute("aria-expanded") !== "true") await user.click(more);
  return screen.getAllByRole("menuitem").map((entry) => entry.textContent);
}
/** Opens the menu and waits for the block lists: Block, or Unblock, takes a press only then. */
async function findBlockItem(user: User, name = "Block") {
  await openMenu(user);
  await waitFor(() => expect(item(name)).toHaveProperty("disabled", false));
  return item(name);
}

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
    expect(await openMenu(user)).toEqual(["Share"]);
    expect(net.fetchFriendship).not.toHaveBeenCalled();
    expect(net.fetchBlocking).not.toHaveBeenCalled();
    expect(net.changeFriendship).not.toHaveBeenCalled();
  });

  it("copies the profile's link from Share in the menu, closes the menu and says so", async () => {
    const user = userEvent.setup();
    const writeText = vi.fn(async () => {});
    vi.stubGlobal("navigator", { ...navigator, clipboard: { writeText } });
    show();

    await openMenu(user);
    await user.click(item("Share"));

    expect(screen.queryByRole("menu")).toBeNull();
    expect(document.activeElement).toBe(button("More actions"));
    expect(writeText).toHaveBeenCalledWith(`${window.location.origin}/profile/${OTHER}`);
    expect(await screen.findByText("Profile link copied")).toBeTruthy();
    expect(screen.getByRole("status").textContent).toBe("Profile link copied");
  });
});

describe("a member's header actions, friendship", () => {
  it("shows the friendship as loading until it is read, signed for the member", async () => {
    let answer: (value: string) => void = () => {};
    net.fetchFriendship.mockReturnValue(new Promise<string>((resolve) => (answer = resolve)));
    show();

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
    expect(button("Request sent Cancel request")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Add friend" })).toBeNull();
  });

  it("cancels a request the member sent", async () => {
    const user = userEvent.setup();
    net.fetchFriendship.mockResolvedValue("request_sent");
    show();
    const cancel = await findButton("Request sent Cancel request");
    net.fetchFriendship.mockResolvedValue("none");

    await user.click(cancel);

    expect(net.changeFriendship).toHaveBeenCalledWith(identity, OTHER, "cancel");
    expect(await findButton("Add friend")).toBeTruthy();
    expect(screen.getByText("Friend request cancelled.")).toBeTruthy();
  });

  it("accepts a request the member received, and then shows Friend with Unfriend", async () => {
    const user = userEvent.setup();
    net.fetchFriendship.mockResolvedValue("request_received");
    show();
    const accept = await findButton("Accept friend request");
    expect(button("Reject friend request")).toBeTruthy();
    net.fetchFriendship.mockResolvedValue("friends");

    await user.click(accept);

    expect(net.changeFriendship).toHaveBeenCalledWith(identity, OTHER, "accept");
    expect(await findButton("Friend Unfriend")).toBeTruthy();
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
    const unfriend = await findButton("Friend Unfriend");
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
    const block = await findBlockItem(user);
    net.fetchBlocking.mockResolvedValue({ blocked: [OTHER], blockedBy: [] });

    await user.click(block);

    expect(net.setBlocked).toHaveBeenCalledWith(identity, OTHER, true);
    expect(await findButton("Blocked Unblock")).toBeTruthy();
    expect(await openMenu(user)).toEqual(["Unblock", "Share"]);
    expect(screen.getByText("Bob has been blocked.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Add friend" })).toBeNull();
  });

  it("unblocks a blocked member, and the friend action comes back", async () => {
    const user = userEvent.setup();
    net.fetchBlocking.mockResolvedValue({ blocked: [OTHER], blockedBy: [] });
    show();
    const unblock = await findButton("Blocked Unblock");
    net.fetchBlocking.mockResolvedValue({ blocked: [], blockedBy: [] });

    await user.click(unblock);

    expect(net.setBlocked).toHaveBeenCalledWith(identity, OTHER, false);
    expect(await findButton("Add friend")).toBeTruthy();
    expect(screen.getByText("Bob has been unblocked.")).toBeTruthy();
  });

  it("unblocks a blocked member from the menu", async () => {
    const user = userEvent.setup();
    net.fetchBlocking.mockResolvedValue({ blocked: [OTHER], blockedBy: [] });
    show();
    const unblock = await findBlockItem(user, "Unblock");
    net.fetchBlocking.mockResolvedValue({ blocked: [], blockedBy: [] });

    await user.click(unblock);

    expect(net.setBlocked).toHaveBeenCalledWith(identity, OTHER, false);
    expect(await screen.findByText("Bob has been unblocked.")).toBeTruthy();
    expect(await openMenu(user)).toEqual(["Block", "Share"]);
  });

  it("offers no friend action to a visitor the member blocked", async () => {
    const user = userEvent.setup();
    net.fetchBlocking.mockResolvedValue({ blocked: [], blockedBy: [OTHER] });
    show();

    await findBlockItem(user);
    expect(screen.queryByRole("button", { name: "Add friend" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Loading friendship" })).toBeNull();
  });

  it("says when the block fails", async () => {
    const user = userEvent.setup();
    net.setBlocked.mockRejectedValue(new Error("500"));
    show();

    await user.click(await findBlockItem(user));

    expect((await screen.findByRole("alert")).textContent).toBe("Bob could not be blocked. Try again.");
    expect(await openMenu(user)).toEqual(["Block", "Share"]);
  });

  it("says when the block lists cannot be read, with no dead Block item, and reads again on Try again", async () => {
    const user = userEvent.setup();
    net.fetchBlocking.mockRejectedValueOnce(new Error("502"));
    show();

    expect(await screen.findByText("Block unavailable")).toBeTruthy();
    expect(await openMenu(user)).toEqual(["Share"]);
    // The friendship was read: its action stays.
    expect(await findButton("Add friend")).toBeTruthy();
    await user.click(button("Try again"));

    await findBlockItem(user);
    expect(screen.queryByText("Block unavailable")).toBeNull();
    expect(net.fetchFriendship).toHaveBeenCalledTimes(1);
  });

  it("reads both again from one Try again when neither the friendship nor the block lists can be read", async () => {
    const user = userEvent.setup();
    net.fetchFriendship.mockRejectedValueOnce(new Error("502"));
    net.fetchBlocking.mockRejectedValueOnce(new Error("502"));
    show();

    expect(await screen.findByText("Friendship unavailable")).toBeTruthy();
    await waitFor(() => expect(client.getQueryState(friendsKeys.blocking(ME))?.status).toBe("error"));
    expect(await openMenu(user)).toEqual(["Share"]);
    expect(screen.getAllByRole("button", { name: "Try again" })).toHaveLength(1);
    await user.click(button("Try again"));

    expect(await findButton("Add friend")).toBeTruthy();
    await findBlockItem(user);
  });
});

describe("a member's header actions, the More actions menu", () => {
  it("opens from the keyboard, moves with the arrow keys, and gives the focus back on Escape", async () => {
    const user = userEvent.setup();
    show();
    const more = button("More actions");
    expect(more.getAttribute("aria-haspopup")).toBe("menu");
    expect(more.getAttribute("aria-expanded")).toBe("false");
    await findBlockItem(user);
    await user.keyboard("{Escape}");
    expect(document.activeElement).toBe(more);

    await user.keyboard("{ArrowDown}");

    expect(more.getAttribute("aria-expanded")).toBe("true");
    expect(document.activeElement).toBe(item("Block"));
    await user.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(item("Share"));
    await user.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(item("Block"));
    await user.keyboard("{ArrowUp}");
    expect(document.activeElement).toBe(item("Share"));
    await user.keyboard("{Escape}");

    expect(screen.queryByRole("menu")).toBeNull();
    expect(document.activeElement).toBe(more);
    expect(net.setBlocked).not.toHaveBeenCalled();
  });

  it("closes on a press outside", async () => {
    const user = userEvent.setup();
    show();
    await openMenu(user);

    await user.click(document.body);

    expect(screen.queryByRole("menu")).toBeNull();
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

  it("loads the signed friends list only after the owner opens it and can retry a failed read", async () => {
    const user = userEvent.setup();
    net.fetchFriendsPage.mockRejectedValueOnce(new Error("offline")).mockResolvedValueOnce({ addresses: [], total: 0 });
    show(<ProfileFriendsCount address={ME} />);

    const trigger = await findButton("17 Friends");
    expect(net.fetchFriendsPage).not.toHaveBeenCalled();
    await user.click(trigger);
    expect(await screen.findByRole("dialog", { name: "Friends" })).toBeTruthy();
    expect(await screen.findByText("Friends are unavailable right now.")).toBeTruthy();
    await user.click(button("Try again"));
    expect(await screen.findByText("You don't have any friends yet.")).toBeTruthy();
    expect(net.fetchFriendsPage.mock.calls[0]?.[0]).toBe(identity);
  });

  it("copies the wallet invite from the owner header", async () => {
    const user = userEvent.setup();
    const writeText = vi.fn(async () => {});
    vi.stubGlobal("navigator", { ...navigator, clipboard: { writeText } });
    show(<ProfileInviteFriends address={ME} />);

    await user.click(button("Invite Friends"));

    expect(writeText).toHaveBeenCalledWith(`${window.location.origin}/invite/${ME}`);
    expect(await findButton("Invite Copied")).toBeTruthy();
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

  it("gives another member's profile the friendship action and the More actions menu", async () => {
    shell(false);

    expect(await findButton("Add friend")).toBeTruthy();
    expect(button("More actions")).toBeTruthy();
    expect(screen.queryByRole("link", { name: "Get a Unique Name" })).toBeNull();
  });

  it("keeps the owner's call to action on their own profile, with their friends count and no friend actions", async () => {
    shell(true);

    expect(await screen.findByText("17 Friends")).toBeTruthy();
    expect(screen.getByRole("link", { name: "Get a Unique Name" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Add friend" })).toBeNull();
    expect(screen.queryByRole("button", { name: "More actions" })).toBeNull();
    expect(net.fetchFriendship).not.toHaveBeenCalled();
  });
});
