import { z } from "zod";

import type { AuthIdentity } from "../auth/types";
import { postJSON, signedGetJSON } from "../catalyst/client";
import { normalizeAddress } from "../catalyst/social/profile";

/** How the signed-in member and another one stand, as the header shows it. */
export type Friendship = "none" | "request_sent" | "request_received" | "friends" | "blocked" | "blocked_by";

/**
 * The node's friendship status numbers (`FriendshipStatus` of the social service protocol). A
 * cancelled, rejected or deleted friendship, or a number this list does not know, is no friendship.
 */
const FRIENDSHIP_BY_STATUS: Readonly<Record<number, Friendship>> = {
  0: "request_sent",
  1: "request_received",
  3: "friends",
  6: "blocked",
  8: "blocked_by",
};

const StatusSchema = z.object({ data: z.object({ status: z.number() }) });
const TotalSchema = z.object({ data: z.object({ total: z.number().int().nonnegative() }) });
const BlockingSchema = z.object({
  data: z.object({ blockedUsers: z.array(z.string()), blockedByUsers: z.array(z.string()) }),
});

type ReadOptions = { signal?: AbortSignal };

const userPath = (address: string) => encodeURIComponent(normalizeAddress(address));

/** Every request here is signed by the member's identity; the node reads the account from the signature. */
export async function fetchFriendship(identity: AuthIdentity, address: string, opts: ReadOptions = {}): Promise<Friendship> {
  const raw = await signedGetJSON<unknown>(`/v1/users/${userPath(address)}/friendship-status`, { identity, signal: opts.signal });
  return FRIENDSHIP_BY_STATUS[StatusSchema.parse(raw).data.status] ?? "none";
}

/** How many friends the member has: the total of a one-row page. */
export async function fetchFriendsCount(identity: AuthIdentity, opts: ReadOptions = {}): Promise<number> {
  const raw = await signedGetJSON<unknown>("/v2/friends", { identity, signal: opts.signal, query: { limit: 1, offset: 0 } });
  return TotalSchema.parse(raw).data.total;
}

/** How many friends the member and another one share. */
export async function fetchMutualFriendsCount(identity: AuthIdentity, address: string, opts: ReadOptions = {}): Promise<number> {
  const raw = await signedGetJSON<unknown>(`/v2/users/${userPath(address)}/mutual-friends`, {
    identity,
    signal: opts.signal,
    query: { limit: 1, offset: 0 },
  });
  return TotalSchema.parse(raw).data.total;
}

export type Blocking = { blocked: string[]; blockedBy: string[] };

/** The wallets the member blocked and the ones that blocked the member, lowercased. */
export async function fetchBlocking(identity: AuthIdentity, opts: ReadOptions = {}): Promise<Blocking> {
  const raw = await signedGetJSON<unknown>("/v1/blocking-status", { identity, signal: opts.signal });
  const { blockedUsers, blockedByUsers } = BlockingSchema.parse(raw).data;
  return { blocked: blockedUsers.map(normalizeAddress), blockedBy: blockedByUsers.map(normalizeAddress) };
}

/** What a member can do about a friendship; each is one signed write. */
export type FriendshipAction = "request" | "cancel" | "accept" | "reject" | "remove";

const REQUEST_INTENTION = { cancel: "cancelled", accept: "accepted", reject: "rejected" } as const;

export async function changeFriendship(identity: AuthIdentity, address: string, action: FriendshipAction): Promise<void> {
  const other = normalizeAddress(address);
  if (action === "request") {
    await postJSON<unknown>("/v1/friendship-requests", { address: other }, { identity });
  } else if (action === "remove") {
    await postJSON<unknown>(`/v1/friends/${userPath(other)}`, undefined, { identity, method: "DELETE" });
  } else {
    await postJSON<unknown>(
      `/v1/friendship-requests/${userPath(other)}`,
      { intention: REQUEST_INTENTION[action] },
      { identity, method: "PATCH" },
    );
  }
}

export async function setBlocked(identity: AuthIdentity, address: string, blocked: boolean): Promise<void> {
  const other = normalizeAddress(address);
  if (blocked) await postJSON<unknown>("/v1/blocks", { address: other }, { identity });
  else await postJSON<unknown>(`/v1/blocks/${userPath(other)}`, undefined, { identity, method: "DELETE" });
}
