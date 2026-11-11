import { useMemo } from "react";
import { skipToken, useInfiniteQuery, useQueries, useQuery } from "@tanstack/react-query";

import type { AuthIdentity } from "@/data/lib/auth/types";
import type { Destination } from "@/data/lib/catalyst/places/destinations";
import {
  browsePlacesQuery,
  browsePageRows,
  favouritePlacesQuery,
  liveFeedQuery,
  myPlacesQuery,
  placeCreatorsQuery,
  placesKeys,
  PLACES_STALE_MS,
  selectLiveRail,
  type BrowseFilters,
  type ExploreSection,
} from "@/data/lib/catalyst/places/discover";
import { hiddenFromBrowse } from "@/data/lib/catalyst/places/destinations";

import { creatorBatches, type CreatorProfiles } from "./cards";

/** The Explore all grid, page by page. */
export function useBrowsePlaces(filters: BrowseFilters, enabled: boolean) {
  const query = useInfiniteQuery({ ...browsePlacesQuery(filters), enabled });
  const pageRows = useMemo(() => browsePageRows(query.data?.pages ?? [], filters.category), [query.data, filters.category]);
  const rows = useMemo(() => pageRows.flat(), [pageRows]);
  return { ...query, rows, pageRows };
}

/** Live Now: the busiest four from the live feed. Not read while a search collapses the section. */
export function useLiveRail(enabled: boolean) {
  const query = useQuery({ ...liveFeedQuery(), enabled });
  const rail = useMemo(() => selectLiveRail(query.data?.data ?? []), [query.data]);
  return { ...query, rail };
}

/** Favourites or My places for the signed-in member; nothing is read without an identity or on Explore all. */
export function useMemberPlaces(section: ExploreSection, identity: AuthIdentity | null) {
  const options =
    identity && section === "favourites" ? favouritePlacesQuery(identity) : identity && section === "my" ? myPlacesQuery(identity) : null;
  const query = useQuery({
    queryKey: options?.queryKey ?? placesKeys.favourites(""),
    queryFn: options?.queryFn ?? skipToken,
    staleTime: PLACES_STALE_MS,
  });
  const rows = useMemo(() => (query.data?.data ?? []).filter((place) => !hiddenFromBrowse(place)), [query.data]);
  return { ...query, rows };
}

/**
 * The faces of the creators on screen, one read per group (Live Now, each browse page, a member list) for the
 * addresses no earlier group asked for, so loading another page reads only that page's new creators. Cards
 * render without faces while a read runs or if it fails.
 */
export function usePlaceCreators(groups: readonly (readonly Destination[])[]): CreatorProfiles {
  const batches = useMemo(() => creatorBatches(groups), [groups]);
  return useQueries({
    queries: batches.map((addresses) => placeCreatorsQuery(addresses)),
    combine: (results) => Object.assign({}, ...results.map((result) => result.data ?? {})) as CreatorProfiles,
  });
}
