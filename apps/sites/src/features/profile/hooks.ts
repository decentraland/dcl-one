import { useCallback } from "react";
import { skipToken, useQuery } from "@tanstack/react-query";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { withCreatorName, type Creations } from "@/data/lib/catalyst/marketplace/index";
import { withContactName, type ProfilePlace } from "@/data/lib/catalyst/places/index";
import {
  canReadProfile,
  creationsQuery,
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
  });
}
