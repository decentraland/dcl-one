import { z } from "zod";

import { getJSON, type GetOptions } from "../client";
import { ApiDataTotalSchema, PlaceRowSchema, WorldRowSchema } from "../generated-schemas/places";
import { reportSchemaDrift } from "../schema-drift";
import { placesApiPath } from "../typed";
import { isEnsRealm, type JumpPlaceRecord } from "./jump";

/** The catalogue lookup behind a place link, safe to run on the server and in the browser. */

const CARD_FIELDS = {
  title: true,
  description: true,
  image: true,
  base_position: true,
  owner: true,
  contact_name: true,
  user_count: true,
  world: true,
  world_name: true,
} as const;
const PlaceRow = PlaceRowSchema.pick(CARD_FIELDS);
const WorldRow = WorldRowSchema.pick(CARD_FIELDS);

/** A row that fails the generated schema is reported as drift and left out, never shown half-parsed. */
function parseRows(kind: "Place" | "World", raw: unknown): JumpPlaceRecord[] {
  const schema = kind === "Place" ? PlaceRow : WorldRow;
  const rows: JumpPlaceRecord[] = [];
  for (const row of ApiDataTotalSchema(z.unknown()).parse(raw).data) {
    const parsed = schema.safeParse(row);
    if (parsed.success) rows.push(parsed.data);
    else reportSchemaDrift(kind, parsed.error.issues);
  }
  return rows;
}
const parsePlaces = (raw: unknown) => parseRows("Place", raw);
const parseWorlds = (raw: unknown) => parseRows("World", raw);

const PLACES = placesApiPath("get", "/api/places");
const WORLDS = placesApiPath("get", "/api/worlds");

/**
 * A World link with a position asks for that World's scene; without one, the World-level record.
 * `/places?names=&positions=` reports the Genesis City parcel's occupancy, so the World's own count
 * from `/worlds` replaces it.
 */
export async function loadJumpPlace(
  target: { lookup: [number, number] | undefined; realm: string | undefined },
  opts: GetOptions = {},
): Promise<JumpPlaceRecord | null> {
  const { lookup, realm } = target;
  const positions = lookup ? lookup.join(",") : undefined;
  if (realm && isEnsRealm(realm)) {
    const names = realm;
    if (!positions) return parseWorlds(await getJSON(WORLDS, { ...opts, query: { names } }))[0] ?? null;
    const [scene] = parsePlaces(await getJSON(PLACES, { ...opts, query: { names, positions } }));
    if (!scene) return null;
    const world = await getJSON(WORLDS, { ...opts, query: { names } })
      .then((raw) => parseWorlds(raw)[0] ?? null)
      .catch(() => null);
    return world?.user_count != null ? { ...scene, user_count: world.user_count } : scene;
  }
  return parsePlaces(await getJSON(PLACES, { ...opts, query: { positions } }))[0] ?? null;
}
