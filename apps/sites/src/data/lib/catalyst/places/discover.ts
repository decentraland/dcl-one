import { infiniteQueryOptions, queryOptions, type DehydratedState } from "@tanstack/react-query";
import { z } from "zod";

import type { AuthIdentity } from "../../auth/types";
import { profileFaceUrl } from "../../auth/profile-face-url";
import { catalystBase } from "../client";
import { fetchDestinations, hiddenFromBrowse, type Destination, type DestinationPage, type DestinationParams } from "./destinations";

/** The /places discovery feed. Client-safe: the loader and the page share these factories. */

export const PLACES_STALE_MS = 30_000;
/** A profile's name and face change rarely; the by-lines of a whole grid share one read. */
export const PLACE_CREATORS_STALE_MS = 5 * 60_000;

/** One destinations page feeds the grid; the endpoint caps `limit` at 100. */
export const BROWSE_LIMIT = 48;
/** Live Now shows one row of four. */
export const LIVE_NOW_LIMIT = 4;
/** The feed lists every highlighted row first, so the live read must comfortably exceed that count to find the busiest four. */
export const LIVE_FEED_LIMIT = 40;
/** Favourites and My places read one page, as source (no pagination past the server maximum). */
export const MEMBER_LIMIT = 100;
/** places-api answers 400 to a search shorter than this, so a shorter one is not sent. */
export const MIN_SEARCH_LENGTH = 3;

export const DISCOVER_CATEGORIES = ["social", "music", "art", "game", "fashion", "education", "shop", "sports", "business"] as const;
export const FEATURED_CATEGORY = "featured";
export type DiscoverCategory = "all" | typeof FEATURED_CATEGORY | (typeof DISCOVER_CATEGORIES)[number];
/** The dropdown's order: All, Featured, then the places-api categories. */
export const CATEGORY_OPTIONS: readonly DiscoverCategory[] = ["all", FEATURED_CATEGORY, ...DISCOVER_CATEGORIES];

export type ExploreSection = "all" | "favourites" | "my";

/** `?category=`: Featured or a known category, case and whitespace insensitive; anything else is All. */
export function parseCategory(raw: string | null | undefined): DiscoverCategory {
  const value = raw?.trim().toLowerCase();
  return CATEGORY_OPTIONS.find((option) => option === value) ?? "all";
}

/** `?tab=`: the two member sections (and the American spelling links carry); anything else is Explore all. */
export function parseSection(raw: string | null | undefined): ExploreSection {
  if (raw === "my") return "my";
  return raw === "favourites" || raw === "favorites" ? "favourites" : "all";
}

/** The search places-api will accept, or "" for none. */
export function effectiveSearch(raw: string | null | undefined): string {
  const value = raw?.trim() ?? "";
  return value.length >= MIN_SEARCH_LENGTH ? value : "";
}

export type BrowseFilters = { category: DiscoverCategory; search: string };

function categoryParams(category: DiscoverCategory): Pick<DestinationParams, "categories" | "only_highlighted"> {
  if (category === "all") return {};
  if (category === FEATURED_CATEGORY) return { only_highlighted: true };
  return { categories: category };
}

type MemberList = "favourites" | "mine";
/** One shape for both member lists, so a reader can switch between them without changing its key type. */
const memberKey = (account: string, list: MemberList): readonly ["private", string, "places", MemberList] =>
  ["private", account.toLowerCase(), "places", list] as const;

export const placesKeys = {
  all: ["places"] as const,
  browse: (filters: BrowseFilters) => ["places", "browse", filters.category, filters.search] as const,
  live: () => ["places", "live"] as const,
  creators: (addresses: readonly string[]) => ["places", "creators", ...addresses] as const,
  favourites: (account: string) => memberKey(account, "favourites"),
  mine: (account: string) => memberKey(account, "mine"),
};

/** The offset after `page`, or undefined once the feed is drained: a short page ends it even when `total` says otherwise. */
export function nextBrowseOffset(page: DestinationPage, offset: number, limit = BROWSE_LIMIT): number | undefined {
  const received = page.received ?? page.data.length;
  const next = offset + received;
  return received < limit || next >= page.total ? undefined : next;
}

/** The Explore all grid: the feed in `most_active` order, one infinite query per category and search. */
export const browsePlacesQuery = (filters: BrowseFilters) =>
  infiniteQueryOptions({
    queryKey: placesKeys.browse(filters),
    staleTime: PLACES_STALE_MS,
    initialPageParam: 0,
    queryFn: ({ pageParam, signal }) =>
      fetchDestinations(
        { limit: BROWSE_LIMIT, offset: pageParam || undefined, search: filters.search || undefined, ...categoryParams(filters.category) },
        { signal },
      ),
    getNextPageParam: (last, _pages, offset) => nextBrowseOffset(last, offset),
  });

/** Live Now's own small read, so a refetch always refreshes the first rows, where the busiest scenes are. */
export const liveFeedQuery = () =>
  queryOptions({
    queryKey: placesKeys.live(),
    staleTime: PLACES_STALE_MS,
    queryFn: ({ signal }) => fetchDestinations({ limit: LIVE_FEED_LIMIT }, { signal }),
  });

/** The member's saved places, signed as them; the key names only their account. */
export const favouritePlacesQuery = (identity: AuthIdentity) =>
  queryOptions({
    queryKey: placesKeys.favourites(identity.signer),
    staleTime: PLACES_STALE_MS,
    queryFn: ({ signal }) => fetchDestinations({ only_favorites: true, limit: MEMBER_LIMIT }, { identity, signal }),
  });

/** The member's own places and worlds, in the catalogue's curated order (no `most_active`), as source. */
export const myPlacesQuery = (identity: AuthIdentity) =>
  queryOptions({
    queryKey: placesKeys.mine(identity.signer),
    staleTime: PLACES_STALE_MS,
    queryFn: ({ signal }) =>
      fetchDestinations({ owner: identity.signer.toLowerCase(), limit: MEMBER_LIMIT, order_by: undefined }, { identity, signal }),
  });

/** Curation overrides the junk heuristic under Featured, as places-api does server-side. */
export function hiddenFromCategory(place: Destination, category: DiscoverCategory): boolean {
  return hiddenFromBrowse(place) && !(category === FEATURED_CATEGORY && place.highlighted);
}

/** The four busiest scenes with at least `minUsers` people, from the live feed. */
export function selectLiveRail(rows: readonly Destination[], minUsers = 0): Destination[] {
  return rows
    .filter((place) => !hiddenFromBrowse(place) && (place.user_count ?? 0) >= minUsers)
    .sort((a, b) => (b.user_count ?? 0) - (a.user_count ?? 0))
    .slice(0, LIVE_NOW_LIMIT);
}

/** The grid rows page by page: duplicates across overlapping pages dropped, junk hidden. */
export function browsePageRows(pages: readonly DestinationPage[], category: DiscoverCategory): Destination[][] {
  const seen = new Set<string>();
  return pages.map((page) =>
    page.data.filter((place) => {
      if (seen.has(place.id) || hiddenFromCategory(place, category)) return false;
      seen.add(place.id);
      return true;
    }),
  );
}

/** The grid rows: every page's rows in order. */
export function browseRows(pages: readonly DestinationPage[], category: DiscoverCategory): Destination[] {
  return browsePageRows(pages, category).flat();
}

const ADDRESS = /^0x[0-9a-f]{40}$/;

/** The wallet a place is credited to: the deployer before the LAND or name owner, and only a wallet-shaped value. */
export function placeCreatorAddress(place: Pick<Destination, "creator_address" | "owner">): string | undefined {
  for (const value of [place.creator_address, place.owner]) {
    const address = value?.trim().toLowerCase();
    if (address && ADDRESS.test(address)) return address;
  }
  return undefined;
}

export type PlaceCreatorProfile = { name: string; avatarUrl?: string };

const ProfilesEnvelope = z.array(
  z
    .object({
      avatars: z.array(
        z
          .object({
            ethAddress: z.string(),
            name: z.string().optional(),
            avatar: z.object({ snapshots: z.object({ face256: z.string().optional() }).passthrough().optional() }).passthrough().optional(),
          })
          .passthrough(),
      ),
    })
    .passthrough(),
);

/** One `POST /lambdas/profiles` for a batch of creators; an address without a profile is left out. */
export async function fetchPlaceCreators(addresses: readonly string[], signal?: AbortSignal): Promise<Record<string, PlaceCreatorProfile>> {
  if (!addresses.length) return {};
  const res = await fetch(`${catalystBase()}/lambdas/profiles`, {
    method: "POST",
    headers: { accept: "application/json", "content-type": "application/json" },
    body: JSON.stringify({ ids: addresses }),
    signal,
  });
  if (!res.ok) {
    // Release the connection before failing: an unread body keeps it busy.
    await res.body?.cancel().catch(() => undefined);
    throw new Error(`Profiles returned ${res.status}`);
  }
  const out: Record<string, PlaceCreatorProfile> = {};
  for (const entry of ProfilesEnvelope.parse(await res.json())) {
    const avatar = entry.avatars[0];
    if (!avatar) continue;
    const face = profileFaceUrl(avatar.avatar?.snapshots?.face256);
    out[avatar.ethAddress.toLowerCase()] = { name: avatar.name ?? "", ...(face ? { avatarUrl: face } : {}) };
  }
  return out;
}

/** Sorted and deduplicated so the same creators share one entry whatever order the cards come in. */
export const placeCreatorsQuery = (addresses: readonly string[]) => {
  const ids = [...new Set(addresses.map((address) => address.toLowerCase()))].sort();
  return queryOptions({
    queryKey: placesKeys.creators(ids),
    staleTime: PLACE_CREATORS_STALE_MS,
    queryFn: ({ signal }) => fetchPlaceCreators(ids, signal),
  });
};

/** The first browse page a loader dehydrated, for the agent markdown of /places. */
export function placesFromDehydrated(state: DehydratedState | null | undefined): Destination[] {
  for (const query of state?.queries ?? []) {
    if (query.queryKey[0] !== "places" || query.queryKey[1] !== "browse") continue;
    const data = query.state.data as { pages?: DestinationPage[] } | undefined;
    return data?.pages?.[0]?.data ?? [];
  }
  return [];
}
