import { afterEach, describe, expect, it, vi } from "vitest";

const client = vi.hoisted(() => ({ signedGetJSON: vi.fn(), postJSON: vi.fn() }));
vi.mock("../catalyst/client", async (original) => ({
  ...(await original<typeof import("../catalyst/client")>()),
  signedGetJSON: client.signedGetJSON,
  postJSON: client.postJSON,
}));

import type { AuthIdentity } from "../auth/types";

import {
  changeFriendship,
  fetchBlocking,
  fetchFriendsCount,
  fetchFriendship,
  fetchMutualFriendsCount,
  setBlocked,
} from "./friends";

const ME = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const OTHER = "0x1111111111111111111111111111111111111111";
const MIXED = "0xAbCdEf1111111111111111111111111111111111";
const identity: AuthIdentity = {
  signer: ME,
  ephemeral: { address: `0x${"c".repeat(40)}`, privateKey: `0x${"12".repeat(32)}` },
  expiration: "2999-01-01T00:00:00.000Z",
  authChain: [],
};

afterEach(() => vi.resetAllMocks());

describe("friends reads", () => {
  it.each([
    [0, "request_sent"],
    [1, "request_received"],
    [3, "friends"],
    [6, "blocked"],
    [8, "blocked_by"],
    // Cancelled, rejected, deleted, none, and a number the node may add later.
    [2, "none"],
    [4, "none"],
    [5, "none"],
    [7, "none"],
    [42, "none"],
  ] as const)("reads the node's friendship status %i as %s", async (status, expected) => {
    client.signedGetJSON.mockResolvedValue({ data: { status } });
    const signal = new AbortController().signal;

    await expect(fetchFriendship(identity, MIXED, { signal })).resolves.toBe(expected);

    expect(client.signedGetJSON).toHaveBeenCalledWith(`/v1/users/${MIXED.toLowerCase()}/friendship-status`, { identity, signal });
  });

  it("refuses a friendship answer without a status, so a failure is never read as no friendship", async () => {
    client.signedGetJSON.mockResolvedValue({ data: {} });

    await expect(fetchFriendship(identity, OTHER)).rejects.toThrow();
  });

  it("counts the member's friends, and the mutual ones, from a one-row page's total", async () => {
    client.signedGetJSON.mockResolvedValue({ data: { results: [{ address: OTHER }], total: 17, page: 1, pages: 17, limit: 1 } });

    await expect(fetchFriendsCount(identity)).resolves.toBe(17);
    await expect(fetchMutualFriendsCount(identity, OTHER)).resolves.toBe(17);

    expect(client.signedGetJSON.mock.calls.map(([path, opts]) => [path, opts.query])).toEqual([
      ["/v2/friends", { limit: 1, offset: 0 }],
      [`/v2/users/${OTHER}/mutual-friends`, { limit: 1, offset: 0 }],
    ]);
  });

  it("reads the member's block lists, lowercased", async () => {
    client.signedGetJSON.mockResolvedValue({ data: { blockedUsers: [MIXED], blockedByUsers: [OTHER] } });

    await expect(fetchBlocking(identity)).resolves.toEqual({ blocked: [MIXED.toLowerCase()], blockedBy: [OTHER] });
    expect(client.signedGetJSON.mock.calls[0]?.[0]).toBe("/v1/blocking-status");
  });
});

describe("friends writes", () => {
  it("sends a friend request with the other member's address", async () => {
    await changeFriendship(identity, MIXED, "request");

    expect(client.postJSON).toHaveBeenCalledWith("/v1/friendship-requests", { address: MIXED.toLowerCase() }, { identity });
  });

  it.each([
    ["cancel", "cancelled"],
    ["accept", "accepted"],
    ["reject", "rejected"],
  ] as const)("answers a request: %s is the intention %s", async (action, intention) => {
    await changeFriendship(identity, OTHER, action);

    expect(client.postJSON).toHaveBeenCalledWith(`/v1/friendship-requests/${OTHER}`, { intention }, { identity, method: "PATCH" });
  });

  it("removes a friend", async () => {
    await changeFriendship(identity, OTHER, "remove");

    expect(client.postJSON).toHaveBeenCalledWith(`/v1/friends/${OTHER}`, undefined, { identity, method: "DELETE" });
  });

  it("blocks with the address in the body and unblocks with it in the path", async () => {
    await setBlocked(identity, OTHER, true);
    await setBlocked(identity, OTHER, false);

    expect(client.postJSON.mock.calls).toEqual([
      ["/v1/blocks", { address: OTHER }, { identity }],
      [`/v1/blocks/${OTHER}`, undefined, { identity, method: "DELETE" }],
    ]);
  });

  it("passes the node's refusal through", async () => {
    client.postJSON.mockRejectedValue(new Error("You cannot send a friendship request to yourself"));

    await expect(changeFriendship(identity, ME, "request")).rejects.toThrow("yourself");
  });
});
