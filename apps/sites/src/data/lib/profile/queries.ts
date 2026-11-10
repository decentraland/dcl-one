import { queryOptions } from "@tanstack/react-query";

import type { AuthIdentity } from "../auth/types";
import { fetchCreations } from "../catalyst/marketplace/index";
import { fetchOwnedNames, fetchOwnedWearables } from "../catalyst/marketplace/account";
import { fetchProfilePlaces } from "../catalyst/places/index";
import { fetchUserBadges } from "../catalyst/social/badges";
import { fetchUserPhotos } from "../catalyst/social/photos";
import { fetchMyCommunities } from "../catalyst/social/profile-communities";
import {
  fetchProfile,
  isEthAddress,
  mapProfile,
  normalizeAddress,
} from "../catalyst/social/profile";
import { mapPhotos } from "./view";

/** Page sizes the profile reads use; the loader shares them until it reads through these factories. */
export const PROFILE_PAGE = 48;
export const PROFILE_PLACES_LIMIT = 100;

/** The Sites root default, declared here so a loader's dehydrated data and the page agree on freshness. */
export const PROFILE_STALE_MS = 30_000;

/** Public keys are `["profile", part, address]`; the member's own reads sit under `["private", account, ...]`. */
export const profileKeys = {
  all: ["profile"] as const,
  profile: (address: string) => ["profile", "profile", normalizeAddress(address)] as const,
  creations: (address: string) => ["profile", "creations", normalizeAddress(address)] as const,
  places: (address: string) => ["profile", "places", normalizeAddress(address)] as const,
  photos: (address: string) => ["profile", "photos", normalizeAddress(address)] as const,
  badges: (address: string) => ["profile", "badges", normalizeAddress(address)] as const,
  ownedWearables: (address: string) =>
    ["profile", "owned-wearables", normalizeAddress(address)] as const,
  ownedNames: (address: string) => ["profile", "owned-names", normalizeAddress(address)] as const,
  myCommunities: (account: string) =>
    ["private", normalizeAddress(account), "profile", "communities"] as const,
};

/** Reads only run for a wallet address; anything else is a profile that cannot exist. */
export function canReadProfile(address: string): boolean {
  return isEthAddress(normalizeAddress(address));
}

export const profileQuery = (address: string) =>
  queryOptions({
    queryKey: profileKeys.profile(address),
    staleTime: PROFILE_STALE_MS,
    queryFn: async ({ signal }) => {
      const avatar = await fetchProfile(address, { signal });
      return avatar ? mapProfile(avatar, address) : null;
    },
  });

export const creationsQuery = (address: string) =>
  queryOptions({
    queryKey: profileKeys.creations(address),
    staleTime: PROFILE_STALE_MS,
    queryFn: ({ signal }) =>
      fetchCreations(normalizeAddress(address), { first: PROFILE_PAGE }, { signal }),
  });

export const placesQuery = (address: string) =>
  queryOptions({
    queryKey: profileKeys.places(address),
    staleTime: PROFILE_STALE_MS,
    queryFn: ({ signal }) =>
      fetchProfilePlaces(normalizeAddress(address), { limit: PROFILE_PLACES_LIMIT }, { signal }),
  });

export const photosQuery = (address: string) =>
  queryOptions({
    queryKey: profileKeys.photos(address),
    staleTime: PROFILE_STALE_MS,
    queryFn: async ({ signal }) =>
      mapPhotos(await fetchUserPhotos(normalizeAddress(address), { signal })),
  });

export const badgesQuery = (address: string) =>
  queryOptions({
    queryKey: profileKeys.badges(address),
    staleTime: PROFILE_STALE_MS,
    queryFn: ({ signal }) => fetchUserBadges(normalizeAddress(address), { signal }),
  });

export const ownedWearablesQuery = (address: string) =>
  queryOptions({
    queryKey: profileKeys.ownedWearables(address),
    staleTime: PROFILE_STALE_MS,
    queryFn: ({ signal }) =>
      fetchOwnedWearables(normalizeAddress(address), { first: PROFILE_PAGE }, { signal }),
  });

export const ownedNamesQuery = (address: string) =>
  queryOptions({
    queryKey: profileKeys.ownedNames(address),
    staleTime: PROFILE_STALE_MS,
    queryFn: ({ signal }) =>
      fetchOwnedNames(normalizeAddress(address), { first: PROFILE_PAGE }, { signal }),
  });

/** The identity signs the request and is never cached; the key names only its account. */
export const myCommunitiesQuery = (identity: AuthIdentity) =>
  queryOptions({
    queryKey: profileKeys.myCommunities(identity.signer),
    staleTime: PROFILE_STALE_MS,
    queryFn: ({ signal }) => fetchMyCommunities(identity, { signal }),
  });
