import { placeViewerOf, type PlaceViewer } from "@web-core/place-view";

import { postJSON, signedGetJSON } from "../client";
import { ApiDataSchema, PlaceRowSchema } from "../generated-schemas/places";
import { placesApiPath } from "../typed";
import type { AuthIdentity } from "../../auth/types";

/** `PATCH /api/places/{id}/favorites`, signed as the visitor; the id is a place's or a world's. */
export function savePlaceFavorite(identity: AuthIdentity, placeId: string, favorites: boolean): Promise<unknown> {
  return postJSON(
    placesApiPath("patch", "/api/places/{entity_id}/favorites", { entity_id: placeId }),
    { favorites },
    { identity, method: "PATCH" },
  );
}

/** `PATCH /api/places/{id}/likes`, signed as the visitor, for a place or a world; `null` clears a like or dislike. */
export function savePlaceLike(identity: AuthIdentity, placeId: string, like: boolean | null): Promise<unknown> {
  return postJSON(
    placesApiPath("patch", "/api/places/{entity_id}/likes", { entity_id: placeId }),
    { like },
    { identity, method: "PATCH" },
  );
}

const ViewerEnvelope = ApiDataSchema(PlaceRowSchema.pick({ user_like: true, user_dislike: true, user_favorite: true }));

// As the node tells the two catalogues apart: a place id is a UUID, any other id is a world's.
const PLACE_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/**
 * `GET /api/places/{id}`, or `GET /api/worlds/{id}` for a world (the place read refuses a world's id), signed as
 * the visitor: the node answers with that account's own like and save.
 */
export async function loadPlaceViewer(
  identity: AuthIdentity,
  placeId: string,
  signal?: AbortSignal,
): Promise<PlaceViewer> {
  const path = PLACE_ID.test(placeId)
    ? placesApiPath("get", "/api/places/{place_id}", { place_id: placeId })
    : placesApiPath("get", "/api/worlds/{world_id}", { world_id: placeId });
  const raw = await signedGetJSON<unknown>(path, { identity, signal });
  return placeViewerOf(ViewerEnvelope.parse(raw).data);
}
