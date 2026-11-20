import { infiniteQueryOptions, queryOptions } from "@tanstack/react-query";

import { fetchCommunityProfiles } from "@ui/data/hooks/useCommunityProfiles";

import type { AuthIdentity } from "../auth/types";
import {
  fetchProfileAssetCount,
  fetchProfileAssets,
  fetchProfileCreations,
  fetchWornCollectibles,
  type AssetCategory,
  type CollectiblePage,
  type CreationsCategory,
  type OptionalAssetCategory,
} from "../catalyst/marketplace/profile-collectibles";
import { fetchFavoritePlaces, fetchProfilePlaces } from "../catalyst/places/profile-places";
import { fetchUserBadges } from "../catalyst/social/badges";
import { fetchPhoto, fetchUserPhotos } from "../catalyst/social/photos";
import { fetchMemberCommunities, fetchMyCommunities } from "../catalyst/social/profile-communities";
import { fetchReferralProgress } from "../catalyst/social/referrals";
import {
  fetchProfile,
  isEthAddress,
  mapProfile,
  normalizeAddress,
} from "../catalyst/social/profile";
import { fetchProfileFlags } from "./flags";

/** Page sizes the profile reads use. Creations, assets and photos load more a page at a time; photos keep the profile site's 24. */
export const PROFILE_COLLECTIBLES_PAGE = 24;
export const PROFILE_PLACES_LIMIT = 100;

/** The Creations tab opens on wearables, as the profile site's does. */
export const DEFAULT_CREATIONS_CATEGORY: CreationsCategory = "wearable";

/** The Sites root default, declared here so a loader's dehydrated data and the page agree on freshness. */
export const PROFILE_STALE_MS = 30_000;

/** The server reads the rollout flags once a minute; the page asks no more often. */
export const PROFILE_FLAGS_STALE_MS = 60_000;

/** Public keys are `["profile", part, address]`; the member's own reads sit under `["private", account, ...]`. */
export const profileKeys = {
  all: ["profile"] as const,
  profile: (address: string) => ["profile", "profile", normalizeAddress(address)] as const,
  /** One infinite list per category; the pages it has loaded live under the same key. */
  creations: (address: string, category: CreationsCategory) =>
    ["profile", "creations", normalizeAddress(address), category] as const,
  /** `all` is the unfiltered list. */
  assets: (address: string, category: AssetCategory | null) =>
    ["profile", "assets", normalizeAddress(address), category ?? "all"] as const,
  assetCount: (address: string, category: OptionalAssetCategory) =>
    ["profile", "asset-count", normalizeAddress(address), category] as const,
  /** The catalogue cards of a set of worn items; the same set, in the same order, is one entry. */
  wornCollectibles: (urns: readonly string[]) => ["profile", "worn-collectibles", [...urns]] as const,
  places: (address: string) => ["profile", "places", normalizeAddress(address)] as const,
  photos: (address: string) => ["profile", "photos", normalizeAddress(address)] as const,
  /** One snapshot by id, for a link to a photo past the pages loaded. */
  photo: (id: string) => ["profile", "photo", id] as const,
  badges: (address: string) => ["profile", "badges", normalizeAddress(address)] as const,
  /** The wallets behind a list of places, sorted so the same set is one entry. */
  placeOwners: (addresses: readonly string[]) =>
    ["profile", "place-owners", [...new Set(addresses.map(normalizeAddress))].sort()] as const,
  communities: (address: string) => ["profile", "communities", normalizeAddress(address)] as const,
  myCommunities: (account: string) =>
    ["private", normalizeAddress(account), "profile", "communities"] as const,
  myFavoritePlaces: (account: string) =>
    ["private", normalizeAddress(account), "profile", "favorite-places"] as const,
  /** The owner's own gallery, private snapshots included; its pages live under the same key. */
  myPhotos: (account: string) => ["private", normalizeAddress(account), "profile", "photos"] as const,
  myReferralProgress: (account: string) =>
    ["private", normalizeAddress(account), "profile", "referral-progress"] as const,
  /** The rollout flags a profile reads; the same for every visitor. */
  flags: () => ["profile", "flags"] as const,
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

/**
 * The next page starts after every row already requested, while the catalogue says there are more.
 * A page the catalogue sends empty ends the list, so a total it overstates cannot loop; a page whose
 * rows all failed to parse does not.
 */
export function nextCollectiblesPage(last: CollectiblePage<unknown>, pages: readonly unknown[]): number | undefined {
  const requested = pages.length * PROFILE_COLLECTIBLES_PAGE;
  return last.received > 0 && requested < last.total ? requested : undefined;
}

export const creationsQuery = (address: string, category: CreationsCategory) =>
  infiniteQueryOptions({
    queryKey: profileKeys.creations(address, category),
    staleTime: PROFILE_STALE_MS,
    initialPageParam: 0,
    queryFn: ({ pageParam, signal }) =>
      fetchProfileCreations(
        normalizeAddress(address),
        { category, first: PROFILE_COLLECTIBLES_PAGE, skip: pageParam },
        { signal },
      ),
    getNextPageParam: nextCollectiblesPage,
  });

export const assetsQuery = (address: string, category: AssetCategory | null) =>
  infiniteQueryOptions({
    queryKey: profileKeys.assets(address, category),
    staleTime: PROFILE_STALE_MS,
    initialPageParam: 0,
    queryFn: ({ pageParam, signal }) =>
      fetchProfileAssets(
        normalizeAddress(address),
        { category: category ?? undefined, first: PROFILE_COLLECTIBLES_PAGE, skip: pageParam },
        { signal },
      ),
    getNextPageParam: nextCollectiblesPage,
  });

export const assetCountQuery = (address: string, category: OptionalAssetCategory) =>
  queryOptions({
    queryKey: profileKeys.assetCount(address, category),
    staleTime: PROFILE_STALE_MS,
    queryFn: ({ signal }) => fetchProfileAssetCount(normalizeAddress(address), category, { signal }),
  });

/** The catalogue cards of the collectibles an avatar wears. */
export const wornCollectiblesQuery = (urns: readonly string[]) => {
  const queryKey = profileKeys.wornCollectibles(urns);
  return queryOptions({
    queryKey,
    staleTime: PROFILE_STALE_MS,
    queryFn: ({ signal }) => fetchWornCollectibles(queryKey[2], { signal }),
  });
};

export const placesQuery = (address: string) =>
  queryOptions({
    queryKey: profileKeys.places(address),
    staleTime: PROFILE_STALE_MS,
    queryFn: ({ signal }) =>
      fetchProfilePlaces(normalizeAddress(address), { limit: PROFILE_PLACES_LIMIT }, { signal }),
  });

/** Whether `identity` signs for the wallet whose profile this is, so it may read that wallet's private data. */
export function signsFor(identity: AuthIdentity | null | undefined, address: string): identity is AuthIdentity {
  return identity != null && normalizeAddress(identity.signer) === normalizeAddress(address);
}

/**
 * A wallet's snapshots, a page at a time. Signed by that wallet's own identity the list includes its
 * private snapshots and sits under the account's private key; otherwise it is the public list anyone
 * sees. Another wallet's identity is never sent, so it cannot widen what a visitor reads.
 */
export const photosQuery = (address: string, identity: AuthIdentity | null = null) => {
  const owner = signsFor(identity, address) ? identity : undefined;
  return infiniteQueryOptions({
    queryKey: owner ? profileKeys.myPhotos(owner.signer) : profileKeys.photos(address),
    staleTime: PROFILE_STALE_MS,
    initialPageParam: 0,
    queryFn: ({ pageParam, signal }) =>
      fetchUserPhotos(
        normalizeAddress(address),
        { limit: PROFILE_COLLECTIBLES_PAGE, offset: pageParam },
        owner ? { signal, identity: owner } : { signal },
      ),
    getNextPageParam: nextCollectiblesPage,
  });
};

export const photoQuery = (id: string) =>
  queryOptions({
    queryKey: profileKeys.photo(id),
    staleTime: PROFILE_STALE_MS,
    queryFn: ({ signal }) => fetchPhoto(id, { signal }),
  });

export const badgesQuery = (address: string) =>
  queryOptions({
    queryKey: profileKeys.badges(address),
    staleTime: PROFILE_STALE_MS,
    queryFn: ({ signal }) => fetchUserBadges(normalizeAddress(address), { signal }),
  });

/** Each owner's display name and face, in one batch read. */
export const placeOwnersQuery = (addresses: readonly string[]) => {
  const queryKey = profileKeys.placeOwners(addresses);
  return queryOptions({
    queryKey,
    staleTime: PROFILE_STALE_MS,
    queryFn: ({ signal }) => fetchCommunityProfiles([...queryKey[2]], { signal }),
  });
};

/** The signed-in member's saved places; signed like the communities read, and keyed by account. */
export const myFavoritePlacesQuery = (identity: AuthIdentity) =>
  queryOptions({
    queryKey: profileKeys.myFavoritePlaces(identity.signer),
    staleTime: PROFILE_STALE_MS,
    queryFn: ({ signal }) => fetchFavoritePlaces(identity, { limit: PROFILE_PLACES_LIMIT }, { signal }),
  });

/**
 * A wallet's communities. Signed by that wallet's own identity it is every community of the member,
 * with its members count and privacy, under the account's private key; otherwise it is the public,
 * listed ones anyone sees. The identity signs the request and is never cached, and another wallet's
 * identity is never sent.
 */
export const communitiesQuery = (address: string, identity: AuthIdentity | null = null) => {
  const owner = signsFor(identity, address) ? identity : undefined;
  return queryOptions({
    queryKey: owner ? profileKeys.myCommunities(owner.signer) : profileKeys.communities(address),
    staleTime: PROFILE_STALE_MS,
    queryFn: ({ signal }) =>
      owner ? fetchMyCommunities(owner, { signal }) : fetchMemberCommunities(normalizeAddress(address), { signal }),
  });
};

/** The member's own referral progress; signed like the communities read, and keyed by account. */
export const referralProgressQuery = (identity: AuthIdentity) =>
  queryOptions({
    queryKey: profileKeys.myReferralProgress(identity.signer),
    staleTime: PROFILE_STALE_MS,
    queryFn: ({ signal }) => fetchReferralProgress(identity, { signal }),
  });

/** The rollout flags: the loader seeds them on a document request, the browser reads them otherwise. */
export const profileFlagsQuery = () =>
  queryOptions({
    queryKey: profileKeys.flags(),
    staleTime: PROFILE_FLAGS_STALE_MS,
    queryFn: ({ signal }) => fetchProfileFlags({ signal }),
  });
