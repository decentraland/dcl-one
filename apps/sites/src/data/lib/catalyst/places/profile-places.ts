import type { AuthIdentity } from "../../auth/types";
import type { GetOptions } from "../client";
import type { Place } from "../schema";
import { fetchDestinations } from "./destinations";

/** A place or world as a profile's Places tab shows it. */
export type ProfilePlace = {
  id: string;
  title: string;
  /** An http(s) image URL, or absent. */
  image?: string;
  /** The place's owner wallet, lowercased; absent when the catalogue does not know it. */
  owner?: string;
  world: boolean;
  /** `x,y` for a Genesis City place. */
  basePosition: string;
  worldName?: string;
};

function httpImage(value: string | null): string | undefined {
  const url = value?.trim() ?? "";
  return /^https?:\/\//i.test(url) ? url : undefined;
}

function toProfilePlace(place: Place): ProfilePlace {
  return {
    id: place.id,
    title: place.title?.trim() ?? "",
    image: httpImage(place.image),
    owner: place.owner?.trim().toLowerCase() || undefined,
    world: place.world,
    basePosition: place.base_position,
    worldName: place.world_name ?? undefined,
  };
}

/**
 * Every place and world a wallet owns, Genesis City scenes and Worlds alike, from the destinations
 * catalogue (one request instead of the profile site's `/places` plus `/worlds` pair).
 */
export async function fetchProfilePlaces(
  address: string,
  params: { limit?: number } = {},
  opts: GetOptions = {},
): Promise<ProfilePlacePage> {
  const page = await fetchDestinations(
    { owner: address.trim().toLowerCase(), limit: params.limit ?? 100 },
    opts,
  );
  return { places: page.data.map(toProfilePlace), total: page.total };
}

/** The signed-in member's saved places and worlds; the node answers for the signing account only. */
export async function fetchFavoritePlaces(
  identity: AuthIdentity,
  params: { limit?: number } = {},
  opts: Pick<GetOptions, "signal" | "base"> = {},
): Promise<ProfilePlacePage> {
  const page = await fetchDestinations(
    { only_favorites: true, limit: params.limit ?? 100 },
    { ...opts, identity },
  );
  return { places: page.data.map(toProfilePlace), total: page.total };
}

/** One read's places and how many the catalogue holds in all; `total` exceeds the list past the page cap. */
export type ProfilePlacePage = { places: ProfilePlace[]; total: number };

/** Where a place's Jump In goes: its world, or its parcel in Genesis City; a world without a name has none. */
export function placeLocation(place: ProfilePlace): string | undefined {
  if (place.world) return place.worldName || undefined;
  return place.basePosition || undefined;
}
