import { useCallback } from "react";
import { skipToken, useQuery } from "@tanstack/react-query";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { withCreatorName, type Creations } from "@/data/lib/catalyst/marketplace/index";
import { withContactName, type ProfilePlace } from "@/data/lib/catalyst/places/index";
import {
  badgesQuery,
  canReadProfile,
  creationsQuery,
  emptyTabsQuery,
  myCommunitiesQuery,
  ownedNamesQuery,
  ownedWearablesQuery,
  photosQuery,
  placesQuery,
  PROFILE_STALE_MS,
  profileKeys,
  profileQuery,
} from "@/data/lib/profile/queries";
import { EMPTY_ASSETS, mapAssets, type ProfileAssets } from "@/data/lib/profile/view";

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

/** Creations are relabelled with the profile's display name, as the creator's own page shows them. */
export function useProfileCreations(address: string, creatorName?: string) {
  const select = useCallback(
    (creations: Creations) => withCreatorName(creations, creatorName),
    [creatorName],
  );
  return useQuery({ ...creationsQuery(address), enabled: canReadProfile(address), select });
}

export function useProfilePlaces(address: string, contactName?: string) {
  const select = useCallback(
    (places: ProfilePlace[]) => withContactName(places, contactName),
    [contactName],
  );
  return useQuery({ ...placesQuery(address), enabled: canReadProfile(address), select });
}

export function useProfileBadges(address: string) {
  return useQuery({ ...badgesQuery(address), enabled: canReadProfile(address) });
}

/** A visitor's view probes which member tabs have a row; the owner's view never asks. */
export function useEmptyTabs(address: string, enabled: boolean) {
  return useQuery({ ...emptyTabsQuery(address), enabled: enabled && canReadProfile(address) });
}

export function useProfilePhotos(address: string) {
  return useQuery({ ...photosQuery(address), enabled: canReadProfile(address) });
}

/** Owned wearables and names load apart, so one failing still shows the other. */
export function useProfileAssets(address: string): {
  data: ProfileAssets;
  isPending: boolean;
  isError: boolean;
} {
  const enabled = canReadProfile(address);
  const wearables = useQuery({ ...ownedWearablesQuery(address), enabled });
  const names = useQuery({ ...ownedNamesQuery(address), enabled });
  const empty = { elements: [], total: 0 };
  return {
    data: enabled
      ? mapAssets(wearables.data ?? empty, names.data ?? empty)
      : EMPTY_ASSETS,
    isPending: enabled && (wearables.isPending || names.isPending),
    isError: wearables.isError || names.isError,
  };
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
