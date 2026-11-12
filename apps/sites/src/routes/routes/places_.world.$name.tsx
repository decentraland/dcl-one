import { loadWorldPlace } from "@/data/lib/catalyst/places/destinations";
import { loadPlaceDetail } from "@/features/explorer/place-detail.server";
import PlaceDetailRoute, { handle, headers } from "./places_.$id";

import type { Route } from "./+types/places_.world.$name";

export { handle, headers };

/** A world's scene page at its own name, as decentraland.org serves it (`/places/world/<name>`). */
export function loader({ request, params, context }: Route.LoaderArgs) {
  const name = params.name.toLowerCase();
  return loadPlaceDetail(request, context, (signal) => loadWorldPlace(name, { signal }), { path: `/places/world/${encodeURIComponent(name)}` });
}

export default PlaceDetailRoute;
