// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});

const net = vi.hoisted(() => ({
  fetchHubCommunities: vi.fn(),
  fetchHubDetail: vi.fn(),
  fetchHubPosts: vi.fn(),
  fetchOwnRequests: vi.fn(),
  fetchCommunityRequests: vi.fn(),
  leaveCommunity: vi.fn(),
}));
const detail = vi.hoisted(() => ({ joinCommunity: vi.fn(), requestToJoin: vi.fn() }));
const auth = vi.hoisted(() => ({ identity: null as null | { signer: string } }));
const signIn = vi.hoisted(() => ({ openSignIn: vi.fn() }));

vi.mock("@/data/lib/catalyst/social/community-hub", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/social/community-hub")>()),
  ...net,
}));
vi.mock("@/data/lib/catalyst/social/community-detail", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/social/community-detail")>()),
  ...detail,
}));
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => ({ identity: auth.identity }) }));
vi.mock("@/features/components/auth/signin-store", () => signIn);
// The sidebar as plain navigation: each group's title, then its links or buttons.
vi.mock("@/features/components/chrome/SitesLayout", () => ({
  default: ({
    children,
    secondaryNavigation = [],
  }: {
    children: ReactNode;
    secondaryNavigation?: { title: string; active?: string | null; items: { id: string; label: ReactNode; href?: string }[]; onSelect?: (id: string) => void }[];
  }) => (
    <>
      {secondaryNavigation.map((group) => (
        <nav key={group.title} aria-label={group.title}>
          {group.items.map((item) =>
            item.href ? (
              <a key={item.id} href={item.href} aria-current={item.id === group.active ? "page" : undefined}>
                {item.label}
              </a>
            ) : (
              <button key={item.id} type="button" aria-current={item.id === group.active ? "page" : undefined} onClick={() => group.onSelect?.(item.id)}>
                {item.label}
              </button>
            ),
          )}
        </nav>
      ))}
      <main>{children}</main>
    </>
  ),
}));

import type { HubCommunity, HubDetail } from "@/data/lib/catalyst/social/community-hub";

import { SocialPage } from "./SocialPage";

const ME = "0x1111111111111111111111111111111111111111";
const club: HubCommunity = {
  id: "club",
  name: "Music Club",
  description: "Live sets every Friday.",
  thumbnailUrl: null,
  visibility: null,
  privacy: "public",
  isLive: false,
  membersCount: 4,
  role: "member",
  ownerName: "Nyx",
};
const river: HubCommunity = { ...club, id: "river", name: "River explorers", privacy: "private", role: null };
const opened = (community: HubCommunity | null, extra: Partial<HubDetail> = {}): HubDetail => ({
  id: community?.id ?? "club",
  community,
  members: [{ memberAddress: ME, name: "Me", role: community?.role ?? "member", profilePictureUrl: null, hasClaimedName: false }],
  places: [],
  ...extra,
});

let client: QueryClient;
function mount(props: { communityId?: string | null; postId?: string | null } = {}) {
  const onSelect = vi.fn();
  const view = render(
    <MemoryRouter>
      <QueryClientProvider client={client}>
        <SocialPage communityId={props.communityId ?? null} postId={props.postId ?? null} onSelect={onSelect} />
      </QueryClientProvider>
    </MemoryRouter>,
  );
  return { onSelect, ...view };
}

beforeEach(() => {
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  auth.identity = { signer: ME };
  net.fetchHubCommunities.mockResolvedValue({ rows: [club, river], total: 2, received: 2 });
  net.fetchHubDetail.mockResolvedValue(opened(club));
  net.fetchHubPosts.mockResolvedValue({ rows: [], total: 0, received: 0 });
  net.fetchOwnRequests.mockResolvedValue([]);
  net.fetchCommunityRequests.mockResolvedValue([]);
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  client.clear();
});

describe("when a member opens the hub", () => {
  it("should list the sections and the member's communities in the sidebar", async () => {
    mount();
    const sections = screen.getByRole("navigation", { name: "Social" });
    expect(within(sections).getByRole("button", { name: "Communities" }).getAttribute("aria-current")).toBe("page");
    expect(within(sections).getByRole("button", { name: "Explore communities" })).toBeTruthy();
    const mine = await screen.findByRole("navigation", { name: "Your communities" });
    expect(within(mine).getByRole("link", { name: /Music Club/ }).getAttribute("href")).toBe("/social?community=club");
    expect(within(mine).queryByRole("link", { name: /River explorers/ })).toBeNull();
  });

  it("should offer invitations and requests only while one is pending", async () => {
    net.fetchOwnRequests.mockResolvedValue([
      { id: "r1", communityId: "river", memberAddress: ME, type: "invite", status: "pending", name: "River explorers" },
    ]);
    const user = userEvent.setup();
    mount();
    const sections = screen.getByRole("navigation", { name: "Social" });
    await user.click(await within(sections).findByRole("button", { name: /Invitations & requests\s*1/ }));
    expect(await screen.findByRole("listitem", { name: "River explorers: invitation" })).toBeTruthy();
  });

  it("should search the communities from Explore", async () => {
    const user = userEvent.setup();
    mount();
    await user.click(within(screen.getByRole("navigation", { name: "Social" })).getByRole("button", { name: "Explore communities" }));
    net.fetchHubCommunities.mockResolvedValue({ rows: [river], total: 1, received: 1 });
    await user.type(screen.getByRole("textbox", { name: "Find a community" }), "river");
    await waitFor(() => expect(net.fetchHubCommunities).toHaveBeenCalledWith(expect.objectContaining({ search: "river" })));
    expect(await screen.findByRole("link", { name: /River explorers/ })).toBeTruthy();
  });

  it("should ask a visitor to sign in to create a community", async () => {
    auth.identity = null;
    const user = userEvent.setup();
    mount();
    await user.click(await screen.findByRole("button", { name: "Create a community" }));
    expect(signIn.openSignIn).toHaveBeenCalledTimes(1);
  });
});

describe("when a community opens in the hub", () => {
  it("should open on its overview with its channels", async () => {
    mount({ communityId: "club" });
    expect(await screen.findByRole("heading", { level: 1, name: "Music Club" })).toBeTruthy();
    const channels = screen.getByRole("navigation", { name: "Music Club" });
    expect(within(channels).getByRole("button", { name: "Overview" }).getAttribute("aria-current")).toBe("page");
    expect(within(channels).getByRole("button", { name: /Members\s*4/ })).toBeTruthy();
    expect(within(channels).queryByRole("button", { name: "Hangouts" })).toBeNull();
    expect(within(channels).queryByRole("button", { name: "Membership requests" })).toBeNull();
  });

  it("should open on its announcements when a post link opened it", async () => {
    mount({ communityId: "club", postId: "p1" });
    expect(await screen.findByRole("heading", { level: 1, name: "Announcements" })).toBeTruthy();
    expect(net.fetchHubPosts).toHaveBeenCalled();
  });

  it("should give a moderator the membership requests and the hangouts", async () => {
    const user = userEvent.setup();
    net.fetchHubDetail.mockResolvedValue(opened({ ...club, role: "moderator" }));
    mount({ communityId: "club" });
    const channels = await screen.findByRole("navigation", { name: "Music Club" });
    await user.click(await within(channels).findByRole("button", { name: "Membership requests" }));
    expect(await screen.findByRole("textbox", { name: "Member wallet address" })).toBeTruthy();
    expect(within(channels).getByRole("button", { name: "Hangouts" })).toBeTruthy();
  });

  it("should keep a private community's channels for its members", async () => {
    const user = userEvent.setup();
    net.fetchHubDetail.mockResolvedValue(opened(river, { members: [] }));
    mount({ communityId: "river" });
    expect(await screen.findByText("Join this community to read its announcements.")).toBeTruthy();
    await user.click(within(screen.getByRole("navigation", { name: "River explorers" })).getByRole("button", { name: /Members/ }));
    expect(screen.getByText("Join this community to view its members.")).toBeTruthy();
  });

  it("should say so when the community cannot be loaded, without showing what it showed before", async () => {
    net.fetchHubDetail.mockRejectedValue(new Error("down"));
    mount({ communityId: "club" });
    expect(await screen.findByText("Couldn't load this community.")).toBeTruthy();
    expect(screen.queryByText("Live sets every Friday.")).toBeNull();
  });

  it("should go back to the sections from the sidebar", async () => {
    const user = userEvent.setup();
    const { onSelect } = mount({ communityId: "club" });
    await screen.findByRole("heading", { level: 1, name: "Music Club" });
    await user.click(within(screen.getByRole("navigation", { name: "Social" })).getByRole("button", { name: "Explore communities" }));
    expect(onSelect).toHaveBeenCalledWith(null);
  });
});

describe("when the reader changes their membership", () => {
  it("should send a visitor to sign in", async () => {
    auth.identity = null;
    const user = userEvent.setup();
    net.fetchHubDetail.mockResolvedValue(opened({ ...club, role: null }));
    mount({ communityId: "club" });
    await user.click(await screen.findByRole("button", { name: "Sign in to join" }));
    expect(signIn.openSignIn).toHaveBeenCalledTimes(1);
  });

  it("should confirm a request to join a private community before sending it", async () => {
    const user = userEvent.setup();
    detail.requestToJoin.mockResolvedValue({});
    net.fetchHubDetail.mockResolvedValue(opened(river, { members: [] }));
    mount({ communityId: "river" });
    await user.click(await screen.findByRole("button", { name: "Request to join" }));
    expect(detail.requestToJoin).not.toHaveBeenCalled();
    const dialog = screen.getByRole("dialog", { name: "Request to join?" });
    await user.click(within(dialog).getByRole("button", { name: "Send request" }));
    await waitFor(() => expect(detail.requestToJoin).toHaveBeenCalledWith(auth.identity, "river"));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("should confirm leaving and keep the dialog open with the reason when it fails", async () => {
    const user = userEvent.setup();
    net.leaveCommunity.mockRejectedValue(Object.assign(new Error("Owners cannot leave"), {}));
    mount({ communityId: "club" });
    await user.click(await screen.findByRole("button", { name: "Joined" }));
    const dialog = screen.getByRole("dialog", { name: "Leave community?" });
    await user.click(within(dialog).getByRole("button", { name: "Leave community" }));
    expect((await within(dialog).findByRole("alert")).textContent).toContain("Owners cannot leave");
    await user.click(within(dialog).getByRole("button", { name: "Stay" }));
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
