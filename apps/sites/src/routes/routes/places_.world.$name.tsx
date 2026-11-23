import { loadWorldPlace } from "@/data/lib/catalyst/places/destinations";
import { loadPlaceDetail } from "@/features/explorer/place-detail.server";
import { placeDetailMeta, worldCard } from "../lib/destination-meta";
import { publicBase } from "../lib/social-meta";
import PlaceDetailRoute, { handle, headers } from "./places_.$id";

import type { Route } from "./+types/places_.world.$name";

export { handle, headers };

export function meta({ loaderData, matches, params }: Route.MetaArgs) {
  const name = params.name.toLowerCase();
  return placeDetailMeta(loaderData, publicBase(matches), worldCard(name), `/places/world/${encodeURIComponent(name)}`);
}

/** A world's scene page at its own name, as decentraland.org serves it (`/places/world/<name>`). */
export function loader({ request, params, context }: Route.LoaderArgs) {
  const name = params.name.toLowerCase();
  return loadPlaceDetail(request, context, (signal) => loadWorldPlace(name, { signal }), { path: `/places/world/${encodeURIComponent(name)}` });
}

export default PlaceDetailRoute;
