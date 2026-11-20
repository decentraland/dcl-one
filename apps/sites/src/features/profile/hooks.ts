import { useCallback } from "react";

import { hashKey, skipToken, useInfiniteQuery, useQuery, useQueryClient } from "@tanstack/react-query";

import type { AuthIdentity } from "@/data/lib/auth/types";
import type { AssetCategory, CreationsCategory } from "@/data/lib/catalyst/marketplace/profile-collectibles";
import {
  assetsQuery,
  canReadProfile,
  badgesQuery,
  communitiesQuery,
  creationsQuery,
  myFavoritePlacesQuery,
  photoQuery,
  photosQuery,
  placeOwnersQuery,
  placesQuery,
  PROFILE_STALE_MS,
  profileFlagsQuery,
  profileKeys,
  profileQuery,
  referralProgressQuery,
  signsFor,
  wornCollectiblesQuery,
} from "@/data/lib/profile/queries";

export function useProfile(address: string) {
  return useQuery({ ...profileQuery(address), enabled: canReadProfile(address) });
}

/**
 * The profile a profile page cannot render without. Any failure with nothing cached is thrown: the
 * route's boundary shows a profile read's failure and hands any other error (a bug) to the root's.
 * A failed background refetch keeps the profile on screen.
 */
export function useRequiredProfile(address: string) {
  return useQuery({
    ...profileQuery(address),
    enabled: canReadProfile(address),
    throwOnError: (_error, query) => query.state.data === undefined,
  });
}

/** The items a wallet created in one category, a page at a time. */
export function useProfileCreations(address: string, category: CreationsCategory) {
  return useInfiniteQuery({ ...creationsQuery(address, category), enabled: canReadProfile(address) });
}

/** The tokens a wallet owns, every category or one, a page at a time. */
export function useProfileAssets(address: string, category: AssetCategory | null) {
  return useInfiniteQuery({ ...assetsQuery(address, category), enabled: canReadProfile(address) });
}

/** The catalogue cards of the collectibles an avatar wears; nothing is requested for an avatar that wears none. */
export function useWornCollectibles(urns: readonly string[]) {
  return useQuery({ ...wornCollectiblesQuery(urns), enabled: urns.length > 0 });
}

export function useProfilePlaces(address: string) {
  return useQuery({ ...placesQuery(address), enabled: canReadProfile(address) });
}

/** The signed-in member's saved places; nothing is requested without an identity or while hidden. */
export function useMyFavoritePlaces(identity: AuthIdentity | null, enabled: boolean) {
  return useQuery({
    queryKey: profileKeys.myFavoritePlaces(identity?.signer ?? ""),
    queryFn: identity && enabled ? myFavoritePlacesQuery(identity).queryFn : skipToken,
    staleTime: PROFILE_STALE_MS,
  });
}

/** Display names and faces of the wallets behind other people's places, read in one batch. */
export function usePlaceOwners(addresses: readonly string[]) {
  return useQuery({ ...placeOwnersQuery(addresses), enabled: addresses.length > 0 });
}

/** A profile's badges; only read for a wallet that has a profile, so a bare address shows none. */
export function useProfileBadges(address: string, hasProfile: boolean) {
  return useQuery({ ...badgesQuery(address), enabled: hasProfile && canReadProfile(address) });
}

/**
 * A profile's snapshots, a page at a time: the owner's signed list (private ones included) with their
 * identity, the public list otherwise. While the signed list first loads, the public one already on
 * screen stays; never the other way round, so a signed-out page never keeps private rows.
 */
export function useProfilePhotos(address: string, identity: AuthIdentity | null) {
  const signed = signsFor(identity, address);
  const publicKey = profileKeys.photos(address);
  return useInfiniteQuery({
    ...photosQuery(address, identity),
    enabled: canReadProfile(address),
    placeholderData: (previous, previousQuery) =>
      signed && previousQuery && hashKey(previousQuery.queryKey) === hashKey(publicKey) ? previous : undefined,
  });
}

/** One snapshot by id; only read while a link names a photo the loaded pages do not hold. */
export function useProfilePhoto(id: string | null) {
  const options = photoQuery(id ?? "");
  return useQuery({ ...options, queryFn: id === null ? skipToken : options.queryFn });
}

/**
 * A profile's communities: the member's own signed list (private ones included) with their identity,
 * the public list otherwise. While the signed list first loads, the public one already on screen
 * stays; never the other way round, so a signed-out page never keeps private rows.
 */
export function useProfileCommunities(address: string, identity: AuthIdentity | null) {
  const signed = signsFor(identity, address);
  const publicKey = profileKeys.communities(address);
  return useQuery({
    ...communitiesQuery(address, identity),
    enabled: canReadProfile(address),
    placeholderData: (previous, previousQuery) =>
      signed && previousQuery && hashKey(previousQuery.queryKey) === hashKey(publicKey) ? previous : undefined,
    // A failed read is reported once; returning to the tab does not retry it on every focus.
    refetchOnWindowFocus: false,
  });
}

/** How many times a failed flags read is made again, each after the query client's retry delay. */
export const PROFILE_FLAGS_RETRIES = 3;

/**
 * The rollout flags the member's own profile reads; nothing is requested for another member's. A
 * failed read is made again, because a page that cannot tell a flag keeps its tab until it can.
 */
export function useProfileFlags(enabled: boolean) {
  return useQuery({ ...profileFlagsQuery(), enabled, retry: PROFILE_FLAGS_RETRIES });
}

/** The member's own referral progress; only read with the identity that signs for this profile. */
export function useReferralProgress(address: string, identity: AuthIdentity | null) {
  const owner = signsFor(identity, address) ? identity : null;
  return useQuery({
    queryKey: profileKeys.myReferralProgress(owner?.signer ?? ""),
    queryFn: owner ? referralProgressQuery(owner).queryFn : skipToken,
    staleTime: PROFILE_STALE_MS,
    // The read marks the count as seen, so it is not repeated on every window focus.
    refetchOnWindowFocus: false,
  });
}

/**
 * Records in the cached progress what the node recorded when it answered: every friend who joined
 * has been seen. The screen calls it once it has shown the tiers, so a return to the tab within the
 * cache time marks none of them new again. The entry's age stays the read's own.
 */
export function useReferralTiersSeen(address: string, identity: AuthIdentity | null) {
  const client = useQueryClient();
  const owner = signsFor(identity, address) ? identity : null;
  return useCallback(() => {
    if (owner === null) return;
    const { queryKey } = referralProgressQuery(owner);
    const progress = client.getQueryData(queryKey);
    if (!progress || progress.viewed >= progress.accepted) return;
    client.setQueryData(
      queryKey,
      { ...progress, viewed: progress.accepted },
      { updatedAt: client.getQueryState(queryKey)?.dataUpdatedAt },
    );
  }, [client, owner]);
}
