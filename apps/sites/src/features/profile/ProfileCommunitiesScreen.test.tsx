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
const net = vi.hoisted(() => ({ fetchMemberCommunities: vi.fn(), fetchMyCommunities: vi.fn(), track: vi.fn() }));
const auth = vi.hoisted(() => ({ state: { identity: null as unknown, isConnected: false, address: null as string | null } }));
vi.mock("@/data/lib/catalyst/social/profile-communities", () => ({
  fetchMemberCommunities: net.fetchMemberCommunities,
  fetchMyCommunities: net.fetchMyCommunities,
}));
vi.mock("@/core/lib/telemetry/track", () => ({ track: net.track }));
vi.mock("@/data/lib/auth/index", () => ({ useAuth: () => auth.state }));

import type { ProfileCommunity } from "@/data/lib/catalyst/social/profile-communities";
import { emptyProfile, type ProfileVM } from "@/data/lib/catalyst/social/profile";
import { profileKeys } from "@/data/lib/profile/queries";

import { ProfileCommunitiesScreen } from "./ProfileCommunitiesScreen";

const A = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const ID = "e99471aa-31c4-4952-abf6-99905445f43b";
const tabs = [
  { id: "overview", label: "Overview", href: `/profile/${A}` },
  { id: "communities", label: "Communities", href: `/profile/${A}/communities` },
];
const identity = { signer: A, ephemeral: { address: "0xeph", privateKey: "0x01" }, expiration: "2999-01-01T00:00:00.000Z", authChain: [] };

const listed = (over: Partial<ProfileCommunity> = {}): ProfileCommunity => ({
  id: ID,
  name: "Builders",
  role: "member",
  membersCount: null,
  privacy: null,
  thumbnailUrl: null,
  ...over,
});
const page = (communities: ProfileCommunity[], total = communities.length) => ({ communities, total });

function show({ own = false, vm = { ...emptyProfile(A), name: "Alice" } as ProfileVM } = {}) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter(
    [{ path: "*", element: <ProfileCommunitiesScreen profile={vm} own={own} tabs={tabs} onTab={vi.fn()} sid="sid-1" /> }],
    { initialEntries: [`/profile/${A}/communities`] },
  );
  render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return client;
}

beforeEach(() => {
  auth.state = { identity: null, isConnected: false, address: null };
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("profile communities, a member's profile", () => {
  it("lists the member's communities in the profile's frame, each a link to its page", async () => {
    net.fetchMemberCommunities.mockResolvedValue(
      page([listed({ role: "owner" }), listed({ id: "c2", name: "Explorers" }), listed({ id: "c3", name: "Wardens", role: "moderator" })]),
    );

    show();

    expect(await screen.findByText("3 communities")).toBeTruthy();
    const owned = screen.getByRole("link", { name: /Builders/ });
    expect(owned.getAttribute("href")).toBe(`/social/communities/${ID}`);
    expect(within(owned.closest("li")!).getByText("Owner")).toBeTruthy();
    expect(within(screen.getByRole("link", { name: /Wardens/ }).closest("li")!).getByText("Moderator")).toBeTruthy();
    // A plain member carries no role mark, and the unsigned read gives no count and no privacy.
    const joined = screen.getByRole("link", { name: "View Explorers" });
    expect(within(joined.closest("li")!).queryByText(/member/i)).toBeNull();
    expect(screen.queryByText("Public")).toBeNull();
    const tab = screen.getByRole("tab", { name: "Communities" });
    expect(tab.getAttribute("aria-selected")).toBe("true");
    expect(screen.getByRole("tabpanel").getAttribute("aria-labelledby")).toBe(tab.id);
    expect(net.fetchMemberCommunities).toHaveBeenCalledWith(A, expect.anything());
    expect(net.fetchMyCommunities).not.toHaveBeenCalled();
  });

  it("reads the community's picture by id and shows the fallback when there is none", async () => {
    net.fetchMemberCommunities.mockResolvedValue(page([listed()]));

    show();

    const card = (await screen.findByRole("link", { name: "View Builders" })).closest("li")!;
    const image = card.querySelector("img") as HTMLImageElement;
    expect(new URL(image.src).pathname).toBe(`/assets-cdn/social/communities/${ID}/raw-thumbnail.png`);

    act(() => {
      image.dispatchEvent(new Event("error"));
    });

    expect(card.querySelector("img")).toBeNull();
    expect(card.querySelector("svg")).toBeTruthy();
  });

  it("says when the member has more communities than the page shows", async () => {
    net.fetchMemberCommunities.mockResolvedValue(page([listed()], 140));

    show();

    expect(await screen.findByText("Showing 1 of 140 communities")).toBeTruthy();
  });

  it("says a member with no community has joined none, with no call to action", async () => {
    net.fetchMemberCommunities.mockResolvedValue(page([]));

    show();

    const panel = within(screen.getByRole("tabpanel", { name: "Communities" }));
    const note = await panel.findByText("This member has not joined any communities yet.");
    expect(note.closest("[role=status]")).toBeTruthy();
    expect(panel.queryByRole("link", { name: "Explore communities" })).toBeNull();
    expect(panel.queryByRole("list")).toBeNull();
  });

  it("announces loading until the read ends", async () => {
    let settle: (value: unknown) => void = () => {};
    net.fetchMemberCommunities.mockReturnValue(new Promise((resolve) => (settle = resolve)));

    show();

    expect(screen.getByText("Loading communities").closest("[role=status]")).toBeTruthy();
    await act(async () => settle(page([listed()])));
    expect(await screen.findByText("1 community")).toBeTruthy();
    expect(screen.queryByText("Loading communities")).toBeNull();
  });

  it("says a failed read failed, reports it once, and reads again on Try again", async () => {
    net.fetchMemberCommunities.mockRejectedValueOnce(new Error("503")).mockResolvedValueOnce(page([listed()]));

    show();

    expect(await screen.findByText("Communities are unavailable right now.")).toBeTruthy();
    // Never the empty message: nothing says the member has no community.
    expect(screen.queryByText("This member has not joined any communities yet.")).toBeNull();
    expect(net.track).toHaveBeenCalledWith("profile_communities_load_failed", { address: A }, expect.anything());

    await userEvent.click(screen.getByRole("button", { name: "Try again" }));

    expect(await screen.findByRole("link", { name: "View Builders" })).toBeTruthy();
    expect(screen.queryByText("Communities are unavailable right now.")).toBeNull();
    expect(net.track.mock.calls.filter(([event]) => event === "profile_communities_load_failed")).toHaveLength(1);
  });

  it("keeps the list on screen when a background re-read fails", async () => {
    net.fetchMemberCommunities.mockResolvedValueOnce(page([listed()])).mockRejectedValueOnce(new Error("503"));
    const client = show();
    await screen.findByRole("link", { name: "View Builders" });

    await act(async () => {
      await client.refetchQueries({ queryKey: profileKeys.communities(A) });
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    expect(client.getQueryState(profileKeys.communities(A))?.status).toBe("error");
    expect(screen.getByRole("link", { name: "View Builders" })).toBeTruthy();
    expect(screen.queryByText("Communities are unavailable right now.")).toBeNull();
    expect(net.track).not.toHaveBeenCalled();
  });
});

describe("profile communities, the member's own profile", () => {
  beforeEach(() => {
    auth.state = { identity, isConnected: true, address: A };
  });

  it("reads the signed list, with each community's members count, privacy and role", async () => {
    net.fetchMyCommunities.mockResolvedValue(
      page([
        listed({ role: "owner", membersCount: 1343, privacy: "private" }),
        listed({ id: "c2", name: "Explorers", membersCount: 1, privacy: "public" }),
      ]),
    );

    show({ own: true });

    const owned = await screen.findByRole("link", { name: /Builders/ });
    expect(within(owned.closest("li")!).getByText("Owner")).toBeTruthy();
    expect(within(owned.closest("li")!).getByText("Private")).toBeTruthy();
    expect(within(owned.closest("li")!).getByText("1.3K Members")).toBeTruthy();
    const joined = screen.getByRole("link", { name: /Explorers/ });
    expect(within(joined.closest("li")!).getByText("Public")).toBeTruthy();
    expect(within(joined.closest("li")!).getByText("1 Members")).toBeTruthy();
    expect(net.fetchMyCommunities).toHaveBeenCalledWith(identity, expect.anything());
    expect(net.fetchMemberCommunities).not.toHaveBeenCalled();
  });

  it("reads the public list while no identity signs for the profile", async () => {
    auth.state = { identity: null, isConnected: true, address: A };
    net.fetchMemberCommunities.mockResolvedValue(page([listed()]));

    show({ own: true });

    expect(await screen.findByRole("link", { name: "Joined Builders" })).toBeTruthy();
    expect(net.fetchMyCommunities).not.toHaveBeenCalled();
  });

  it("invites a member with no community into Decentraland to find one", async () => {
    net.fetchMyCommunities.mockResolvedValue(page([]));

    show({ own: true });

    const title = "No communities yet";
    expect((await screen.findByText(title, { selector: "span" })).closest("[role=status]")).toBeTruthy();
    expect(screen.getByText(title, { selector: "p" })).toBeTruthy();
    const cta = screen.getByRole("link", { name: "Explore communities" });
    await waitFor(() => expect(cta.getAttribute("href")).toMatch(/\/play\/?$/));
    expect(screen.queryByText("This member has not joined any communities yet.")).toBeNull();
  });
});
