import { placeViewerOf, type PlaceViewer } from "@ui/data/catalyst/places";

import { postJSON, signedGetJSON } from "../client";
import { ApiDataSchema, PlaceRowSchema } from "../generated-schemas/places";
import { placesApiPath } from "../typed";
import type { AuthIdentity } from "../../auth/types";

/** `PATCH /api/places/{id}/favorites`, signed as the visitor. */
export function savePlaceFavorite(identity: AuthIdentity, placeId: string, favorites: boolean): Promise<unknown> {
  return postJSON(
    placesApiPath("patch", "/api/places/{entity_id}/favorites", { entity_id: placeId }),
    { favorites },
    { identity, method: "PATCH" },
  );
}

/** `PATCH /api/places/{id}/likes`, signed as the visitor; `null` clears a like or dislike. */
export function savePlaceLike(identity: AuthIdentity, placeId: string, like: boolean | null): Promise<unknown> {
  return postJSON(
    placesApiPath("patch", "/api/places/{entity_id}/likes", { entity_id: placeId }),
    { like },
    { identity, method: "PATCH" },
  );
}

const ViewerEnvelope = ApiDataSchema(PlaceRowSchema.pick({ user_like: true, user_dislike: true, user_favorite: true }));

/** `GET /api/places/{id}`, signed as the visitor: the node answers with that account's own like and save. */
export async function loadPlaceViewer(
  identity: AuthIdentity,
  placeId: string,
  signal?: AbortSignal,
): Promise<PlaceViewer> {
  const raw = await signedGetJSON<unknown>(
    placesApiPath("get", "/api/places/{place_id}", { place_id: placeId }),
    { identity, signal },
  );
  return placeViewerOf(ViewerEnvelope.parse(raw).data);
}
