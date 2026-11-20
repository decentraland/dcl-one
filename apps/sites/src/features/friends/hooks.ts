import { skipToken, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { changeFriendship, setBlocked, type FriendshipAction } from "@/data/lib/friends/friends";
import {
  blockingQuery,
  FRIENDS_STALE_MS,
  friendsCountQuery,
  friendshipQuery,
  friendsKeys,
  invalidateFriendship,
  mutualFriendsCountQuery,
} from "@/data/lib/friends/queries";

/** How the signed-in member and `address` stand; nothing is requested without an identity. */
export function useFriendship(identity: AuthIdentity | null, address: string) {
  return useQuery({
    queryKey: friendsKeys.friendship(identity?.signer ?? "", address),
    queryFn: identity ? friendshipQuery(identity, address).queryFn : skipToken,
    staleTime: FRIENDS_STALE_MS,
  });
}

/** How many friends the signed-in member has. */
export function useFriendsCount(identity: AuthIdentity | null) {
  return useQuery({
    queryKey: friendsKeys.count(identity?.signer ?? ""),
    queryFn: identity ? friendsCountQuery(identity).queryFn : skipToken,
    staleTime: FRIENDS_STALE_MS,
  });
}

/** How many friends the signed-in member shares with `address`. */
export function useMutualFriendsCount(identity: AuthIdentity | null, address: string) {
  return useQuery({
    queryKey: friendsKeys.mutualCount(identity?.signer ?? "", address),
    queryFn: identity ? mutualFriendsCountQuery(identity, address).queryFn : skipToken,
    staleTime: FRIENDS_STALE_MS,
  });
}

/** Whether the signed-in member blocked `address`, or is blocked by it, from the member's block lists. */
export function useBlocking(identity: AuthIdentity | null) {
  return useQuery({
    queryKey: friendsKeys.blocking(identity?.signer ?? ""),
    queryFn: identity ? blockingQuery(identity).queryFn : skipToken,
    staleTime: FRIENDS_STALE_MS,
  });
}

/** Sends, cancels, accepts or rejects a request, or removes a friend; then every read it changes is read again. */
export function useChangeFriendship(identity: AuthIdentity | null, address: string) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (action: FriendshipAction) => {
      if (!identity) throw new Error("sign in to change a friendship");
      return changeFriendship(identity, address, action);
    },
    // Also after a refusal: the other member may have acted first, so what the page shows is stale.
    onSettled: () => (identity ? invalidateFriendship(client, identity.signer, address) : undefined),
  });
}

/** Blocks or unblocks `address`; then every read it changes is read again. */
export function useSetBlocked(identity: AuthIdentity | null, address: string) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (blocked: boolean) => {
      if (!identity) throw new Error("sign in to block a member");
      return setBlocked(identity, address, blocked);
    },
    onSettled: () => (identity ? invalidateFriendship(client, identity.signer, address) : undefined),
  });
}
