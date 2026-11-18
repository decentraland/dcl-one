import { skipToken, useInfiniteQuery, useQuery } from "@tanstack/react-query";

import type { AuthIdentity } from "@/data/lib/auth/types";
import type { AssetCategory, CreationsCategory } from "@/data/lib/catalyst/marketplace/profile-collectibles";
import {
  assetsQuery,
  canReadProfile,
  badgesQuery,
  creationsQuery,
  myCommunitiesQuery,
  myFavoritePlacesQuery,
  photosQuery,
  placeOwnersQuery,
  placesQuery,
  PROFILE_STALE_MS,
  profileKeys,
  profileQuery,
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

export function useProfilePhotos(address: string) {
  return useQuery({ ...photosQuery(address), enabled: canReadProfile(address) });
}

/** The signed-in member's communities; nothing is requested without an identity. */
export function useMyCommunities(identity: AuthIdentity | null, enabled = true) {
  const options = identity ? myCommunitiesQuery(identity) : null;
  return useQuery({
    queryKey: options?.queryKey ?? profileKeys.myCommunities(""),
    queryFn: identity && enabled ? myCommunitiesQuery(identity).queryFn : skipToken,
    staleTime: PROFILE_STALE_MS,
    // A failed read is reported once; returning to the tab does not retry it on every focus.
    refetchOnWindowFocus: false,
  });
}
