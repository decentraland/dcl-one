import { infiniteQueryOptions, queryOptions, type QueryClient } from "@tanstack/react-query";

import { fetchMemberProfiles } from "../catalyst/social/member-profiles";

import type { AuthIdentity } from "../auth/types";
import { normalizeAddress } from "../catalyst/social/profile";
import { fetchBlocking, fetchFriendsCount, fetchFriendsPage, fetchFriendship, fetchMutualFriendsCount } from "./friends";

/** The Sites root default, declared here so every friends read agrees on freshness. */
export const FRIENDS_STALE_MS = 30_000;

/** Every friends read is the signed-in member's own, so all keys sit under `["private", account, "friends", ...]`. */
export const friendsKeys = {
  all: (account: string) => ["private", normalizeAddress(account), "friends"] as const,
  friendship: (account: string, address: string) =>
    ["private", normalizeAddress(account), "friends", "friendship", normalizeAddress(address)] as const,
  list: (account: string) => ["private", normalizeAddress(account), "friends", "list"] as const,
  count: (account: string) => ["private", normalizeAddress(account), "friends", "count"] as const,
  mutualCount: (account: string, address: string) =>
    ["private", normalizeAddress(account), "friends", "mutual-count", normalizeAddress(address)] as const,
  blocking: (account: string) => ["private", normalizeAddress(account), "friends", "blocking"] as const,
};

/** The identity signs each request and is never cached; the keys name only its account. */
export const friendshipQuery = (identity: AuthIdentity, address: string) =>
  queryOptions({
    queryKey: friendsKeys.friendship(identity.signer, address),
    staleTime: FRIENDS_STALE_MS,
    queryFn: ({ signal }) => fetchFriendship(identity, address, { signal }),
  });

export const friendsCountQuery = (identity: AuthIdentity) =>
  queryOptions({
    queryKey: friendsKeys.count(identity.signer),
    staleTime: FRIENDS_STALE_MS,
    queryFn: ({ signal }) => fetchFriendsCount(identity, { signal }),
  });

const FRIENDS_PAGE_SIZE = 20;

export const friendsListQuery = (identity: AuthIdentity) =>
  infiniteQueryOptions({
    queryKey: friendsKeys.list(identity.signer),
    staleTime: FRIENDS_STALE_MS,
    initialPageParam: 0,
    queryFn: ({ pageParam, signal }) =>
      fetchFriendsPage(identity, { offset: pageParam, limit: FRIENDS_PAGE_SIZE }, { signal }),
    getNextPageParam: (last, _pages, offset) =>
      last.addresses.length > 0 && offset + FRIENDS_PAGE_SIZE < last.total ? offset + FRIENDS_PAGE_SIZE : undefined,
  });

/** The profile data is public, but a batch of the member's friends reveals their private list. */
export const friendProfilesQuery = (identity: AuthIdentity, addresses: readonly string[]) => {
  const sorted = [...new Set(addresses.map(normalizeAddress))].sort();
  return queryOptions({
    queryKey: [...friendsKeys.list(identity.signer), "profiles", sorted] as const,
    staleTime: FRIENDS_STALE_MS,
    enabled: sorted.length > 0,
    queryFn: ({ signal }) => fetchMemberProfiles(sorted, { signal }),
  });
};

export const mutualFriendsCountQuery = (identity: AuthIdentity, address: string) =>
  queryOptions({
    queryKey: friendsKeys.mutualCount(identity.signer, address),
    staleTime: FRIENDS_STALE_MS,
    queryFn: ({ signal }) => fetchMutualFriendsCount(identity, address, { signal }),
  });

export const blockingQuery = (identity: AuthIdentity) =>
  queryOptions({
    queryKey: friendsKeys.blocking(identity.signer),
    staleTime: FRIENDS_STALE_MS,
    queryFn: ({ signal }) => fetchBlocking(identity, { signal }),
  });

/**
 * What a friendship or block write changes: how the two members stand, the member's friends and
 * mutual friends counts, and the block lists. One helper, so no write leaves one of them stale.
 */
export function invalidateFriendship(client: QueryClient, account: string, address: string): Promise<void> {
  return Promise.all([
    client.invalidateQueries({ queryKey: friendsKeys.friendship(account, address) }),
    client.invalidateQueries({ queryKey: friendsKeys.count(account) }),
    client.invalidateQueries({ queryKey: friendsKeys.list(account) }),
    client.invalidateQueries({ queryKey: friendsKeys.mutualCount(account, address) }),
    client.invalidateQueries({ queryKey: friendsKeys.blocking(account) }),
  ]).then(() => undefined);
}
