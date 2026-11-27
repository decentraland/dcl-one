// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});

const net = vi.hoisted(() => ({
  fetchHubPosts: vi.fn(),
  publishPost: vi.fn(),
  editPost: vi.fn(),
  leaveCommunity: vi.fn(),
  changeMember: vi.fn(),
  fetchOwnRequests: vi.fn(),
  decideRequest: vi.fn(),
}));
const detail = vi.hoisted(() => ({ joinCommunity: vi.fn(), requestToJoin: vi.fn() }));
vi.mock("@/data/lib/catalyst/social/community-hub", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/social/community-hub")>()),
  ...net,
}));
vi.mock("@/data/lib/catalyst/social/community-detail", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/social/community-detail")>()),
  ...detail,
}));

import type { AuthIdentity } from "@/data/lib/auth/types";
import { CatalystError } from "@/data/lib/catalyst/client";
import type { HubCommunity, HubRequest } from "@/data/lib/catalyst/social/community-hub";
import { hubKeys } from "@/data/lib/community/hub-queries";
import { communityKeys } from "@/data/lib/community/queries";

import { useHubMemberChange, useHubMembership, useHubPosts, useHubRequests } from "./useHub";

const identity = { signer: "0xABC" } as AuthIdentity;
const club: HubCommunity = {
  id: "club",
  name: "Club",
  description: null,
  thumbnailUrl: null,
  visibility: null,
  privacy: "public",
  isLive: false,
  membersCount: 2,
  role: null,
  ownerName: null,
};

let client: QueryClient;
const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;

beforeEach(() => {
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  net.fetchHubPosts.mockResolvedValue({ rows: [], total: 0, received: 0 });
});
afterEach(() => {
  vi.resetAllMocks();
  client.clear();
});

describe("useHubPosts", () => {
  it("should publish a retry after a lost answer with the same key, and a new post with a new one", async () => {
    net.publishPost.mockRejectedValueOnce(new CatalystError("network error", "/posts")).mockResolvedValue(undefined);
    const { result } = renderHook(() => useHubPosts(identity, "club"), { wrapper });
    await waitFor(() => expect(result.current.pending).toBe(false));
    await act(async () => {
      await expect(result.current.publish("Once only")).rejects.toThrow("The post could not be updated. Try again.");
    });
    await act(() => result.current.publish("Once only"));
    await act(() => result.current.publish("Once only"));
    const keys = net.publishPost.mock.calls.map((call) => call[3]);
    expect(keys[0]).toBe(keys[1]);
    expect(keys[2]).not.toBe(keys[0]);
  });

  it("should explain a refused change and read the feed again", async () => {
    net.editPost.mockRejectedValue(new CatalystError("Unauthorized", "/posts", 401, true));
    const { result } = renderHook(() => useHubPosts(identity, "club"), { wrapper });
    await waitFor(() => expect(result.current.pending).toBe(false));
    await act(async () => {
      await expect(result.current.edit({ id: "p" }, "New words")).rejects.toThrow(
        "You no longer have permission to change posts in this community.",
      );
    });
    expect(net.fetchHubPosts).toHaveBeenCalledTimes(2);
  });

  it("should show no cached posts after a refused read", async () => {
    net.fetchHubPosts.mockResolvedValueOnce({ rows: [{ id: "p", authorAddress: "0x1", content: "Hi", createdAt: null, likesCount: 0 }], total: 1, received: 1 });
    const { result } = renderHook(() => useHubPosts(identity, "club"), { wrapper });
    await waitFor(() => expect(result.current.posts).toHaveLength(1));
    net.fetchHubPosts.mockRejectedValueOnce(new CatalystError("Membership required", "/posts", 401, true));
    await act(() => client.invalidateQueries({ queryKey: hubKeys.posts("0xabc", "club") }));
    await waitFor(() => expect(result.current.error).toBe("Membership required"));
    expect(result.current.posts).toEqual([]);
    expect(result.current.hasMore).toBe(false);
  });
});

describe("useHubMembership", () => {
  it("should read the hub and the community's own page again after a join", async () => {
    detail.joinCommunity.mockResolvedValue(undefined);
    client.setQueryData(hubKeys.communities("0xabc"), { pages: [], pageParams: [] });
    client.setQueryData(communityKeys.detail("club"), {});
    const { result } = renderHook(() => useHubMembership(identity), { wrapper });
    await act(() => result.current.change({ community: club, kind: "join" }));
    expect(detail.joinCommunity).toHaveBeenCalledWith(identity, "club");
    expect(client.getQueryState(hubKeys.communities("0xabc"))?.isInvalidated).toBe(true);
    expect(client.getQueryState(communityKeys.detail("club"))?.isInvalidated).toBe(true);
  });

  it("should ask to join a private community instead of joining it", async () => {
    detail.requestToJoin.mockResolvedValue({});
    const { result } = renderHook(() => useHubMembership(identity), { wrapper });
    await act(() => result.current.change({ community: { ...club, privacy: "private" }, kind: "join" }));
    expect(detail.requestToJoin).toHaveBeenCalledWith(identity, "club");
    expect(detail.joinCommunity).not.toHaveBeenCalled();
  });

  it("should report a failed leave and change nothing", async () => {
    net.leaveCommunity.mockRejectedValue(new CatalystError("Owners cannot leave", "/members", 400, true));
    const { result } = renderHook(() => useHubMembership(identity), { wrapper });
    await act(async () => {
      await expect(result.current.change({ community: club, kind: "leave" })).rejects.toBeTruthy();
    });
    await waitFor(() => expect(result.current.error).toBe("Owners cannot leave"));
    expect(result.current.pending).toBeNull();
  });

  it("should refuse a visitor", async () => {
    const { result } = renderHook(() => useHubMembership(null), { wrapper });
    await act(async () => {
      await expect(result.current.change({ community: club, kind: "join" })).rejects.toThrow("Sign in to manage membership.");
    });
  });
});

describe("useHubRequests", () => {
  const invite: HubRequest = { id: "r1", communityId: "club", memberAddress: "0xabc", type: "invite", status: "pending", name: "Club" };

  it("should count the pending requests and answer one", async () => {
    net.fetchOwnRequests.mockResolvedValue([invite, { ...invite, id: "r2", status: "accepted" }]);
    net.decideRequest.mockResolvedValue(undefined);
    const { result } = renderHook(() => useHubRequests(identity, null, false), { wrapper });
    await waitFor(() => expect(result.current.pendingOwn).toEqual([invite]));
    act(() => result.current.own.decide(invite, "accepted"));
    await waitFor(() => expect(net.decideRequest).toHaveBeenCalledWith(identity, invite, "accepted"));
  });

  it("should not read requests for a visitor", () => {
    const { result } = renderHook(() => useHubRequests(null, "club", false), { wrapper });
    expect(net.fetchOwnRequests).not.toHaveBeenCalled();
    expect(result.current.own.pending).toBe(false);
  });

  it("should know the member is waiting for an answer to join", async () => {
    net.fetchOwnRequests.mockResolvedValue([{ ...invite, type: "request_to_join" }]);
    const { result } = renderHook(() => useHubRequests(identity, null, false), { wrapper });
    await waitFor(() => expect(result.current.requested("club")).toBe(true));
    expect(result.current.requested("other")).toBe(false);
  });
});

describe("useHubMemberChange", () => {
  it("should explain a refused removal", async () => {
    net.changeMember.mockRejectedValue(new CatalystError("cannot kick", "/members", 401, true));
    const { result } = renderHook(() => useHubMemberChange(identity, "club"), { wrapper });
    await expect(
      result.current({ memberAddress: "0x2", role: "member", name: null, profilePictureUrl: null, hasClaimedName: false }, "remove"),
    ).rejects.toThrow("You don't have permission to change this member.");
  });
});
