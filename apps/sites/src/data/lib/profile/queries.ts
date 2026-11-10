import { queryOptions } from "@tanstack/react-query";

import type { AuthIdentity } from "../auth/types";
import { fetchCreations } from "../catalyst/marketplace/index";
import { fetchOwnedNames, fetchOwnedWearables } from "../catalyst/marketplace/account";
import { fetchProfilePlaces } from "../catalyst/places/index";
import { fetchUserPhotos } from "../catalyst/social/photos";
import { fetchMyCommunities } from "../catalyst/social/profile-communities";
import {
  fetchProfile,
  isEthAddress,
  mapProfile,
  normalizeAddress,
} from "../catalyst/social/profile";
import { mapPhotos } from "./view";

const PAGE = 48;
const PLACES_LIMIT = 100;

/** Public keys are `["profile", part, address]`; the member's own reads sit under `["private", account, ...]`. */
export const profileKeys = {
  all: ["profile"] as const,
  profile: (address: string) => ["profile", "profile", normalizeAddress(address)] as const,
  creations: (address: string) => ["profile", "creations", normalizeAddress(address)] as const,
  places: (address: string) => ["profile", "places", normalizeAddress(address)] as const,
  photos: (address: string) => ["profile", "photos", normalizeAddress(address)] as const,
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
    queryFn: async ({ signal }) => {
      const avatar = await fetchProfile(address, { signal });
      return avatar ? mapProfile(avatar, address) : null;
    },
  });

export const creationsQuery = (address: string) =>
  queryOptions({
    queryKey: profileKeys.creations(address),
    queryFn: ({ signal }) =>
      fetchCreations(normalizeAddress(address), { first: PAGE }, { signal }),
  });

export const placesQuery = (address: string) =>
  queryOptions({
    queryKey: profileKeys.places(address),
    queryFn: ({ signal }) =>
      fetchProfilePlaces(normalizeAddress(address), { limit: PLACES_LIMIT }, { signal }),
  });

export const photosQuery = (address: string) =>
  queryOptions({
    queryKey: profileKeys.photos(address),
    queryFn: async ({ signal }) =>
      mapPhotos(await fetchUserPhotos(normalizeAddress(address), { signal })),
  });

export const ownedWearablesQuery = (address: string) =>
  queryOptions({
    queryKey: profileKeys.ownedWearables(address),
    queryFn: ({ signal }) =>
      fetchOwnedWearables(normalizeAddress(address), { first: PAGE }, { signal }),
  });

export const ownedNamesQuery = (address: string) =>
  queryOptions({
    queryKey: profileKeys.ownedNames(address),
    queryFn: ({ signal }) =>
      fetchOwnedNames(normalizeAddress(address), { first: PAGE }, { signal }),
  });

/** The identity signs the request and is never cached; the key names only its account. */
export const myCommunitiesQuery = (identity: AuthIdentity) =>
  queryOptions({
    queryKey: profileKeys.myCommunities(identity.signer),
    queryFn: ({ signal }) => fetchMyCommunities(identity, { signal }),
  });
