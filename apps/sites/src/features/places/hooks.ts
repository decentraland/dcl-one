import { useMemo } from "react";
import { skipToken, useInfiniteQuery, useQuery } from "@tanstack/react-query";

import type { AuthIdentity } from "@/data/lib/auth/types";
import type { Destination } from "@/data/lib/catalyst/places/destinations";
import {
  browsePlacesQuery,
  browseRows,
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

import { creatorAddresses } from "./cards";

/** The Explore all grid, page by page. */
export function useBrowsePlaces(filters: BrowseFilters, enabled: boolean) {
  const query = useInfiniteQuery({ ...browsePlacesQuery(filters), enabled });
  const rows = useMemo(() => browseRows(query.data?.pages ?? [], filters.category), [query.data, filters.category]);
  return { ...query, rows };
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

/** The faces of every creator on screen, in one read; cards render without them while it runs or if it fails. */
export function usePlaceCreators(places: readonly Destination[]) {
  const addresses = useMemo(() => creatorAddresses(places), [places]);
  // The previous faces stay while a longer list (another page) is read.
  const query = useQuery({ ...placeCreatorsQuery(addresses), enabled: addresses.length > 0, placeholderData: (previous) => previous });
  return query.data ?? {};
}
