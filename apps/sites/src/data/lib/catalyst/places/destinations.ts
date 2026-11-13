import { z } from "zod";
import { readCopy, type CopyStamp } from "@ui/data/copyAsOf";
import type { AuthIdentity } from "../../auth/types";
import { getJSON, signedGetJSON, type GetOptions } from "../client";
import { ApiDataTotalSchema, DestinationSchema, WorldRowSchema } from "../generated-schemas/places";
import type { Place } from "../schema";
import { reportSchemaDrift } from "../schema-drift";

export type DestinationParams = {
  limit?: number; offset?: number; categories?: string; search?: string; owner?: string;
  only_favorites?: boolean; only_highlighted?: boolean; world_names?: string;
  order_by?: "most_active";
};
/** A destinations row: the place, plus what only this feed carries (the running event, the deployer, the member's save). */
export type Destination = Place & {
  live: boolean;
  live_event_name: string | null;
  creator_address: string | null;
  user_favorite: boolean;
};
export type DestinationPage = { data: Destination[]; total: number; copy: CopyStamp | null; received?: number };

function toPlace(row: z.infer<typeof DestinationSchema>): Destination {
  return {
    id: row.id, title: row.title, description: row.description, image: row.image, owner: row.owner,
    positions: row.positions, base_position: row.base_position, updated_at: row.updated_at, created_at: row.created_at,
    contact_name: row.contact_name, categories: row.categories, highlighted: row.highlighted, highlighted_image: row.highlighted_image,
    user_count: row.user_count, user_visits: row.user_visits, favorites: row.favorites, likes: row.likes, like_rate: row.like_rate,
    world: row.world, world_name: row.world_name,
    live: row.live === true, live_event_name: row.live_event_name?.trim() || null,
    creator_address: row.creator_address, user_favorite: row.user_favorite,
  };
}

export async function fetchDestinations(params: DestinationParams = {}, options: GetOptions & { identity?: AuthIdentity } = {}): Promise<DestinationPage> {
  if (params.only_favorites && !options.identity) throw new Error("Sign in to see your favourites.");
  const query = { order_by: "most_active", ...params, with_realms_detail: true, with_live_events: true };
  const path = "/places/api/destinations";
  const raw = options.identity
    ? await signedGetJSON(path, { identity: options.identity, base: options.base, signal: options.signal, query })
    : await getJSON(path, { ...options, query });
  const envelope = ApiDataTotalSchema(z.unknown()).parse(raw);
  const data: Destination[] = [];
  for (const row of envelope.data) {
    const parsed = DestinationSchema.safeParse(row);
    if (parsed.success) data.push(toPlace(parsed.data));
    else reportSchemaDrift("Destination", parsed.error.issues);
  }
  return { data, total: envelope.total, copy: readCopy(raw), received: envelope.data.length };
}

/** Hide unfinished catalogue entries on the default public view; explicit searches and personal views keep their matches. */
export function hiddenFromBrowse(place: Place): boolean {
  const title = place.title?.trim().toLowerCase() ?? "";
  if (title === "empty" || title.includes("interactive-text")) return true;
  if (!place.world && place.title?.startsWith("Road at ") && place.categories.length === 0) return true;
  const image = place.image ?? "";
  const thumbnail = image.split("/").pop();
  const placeholder = !image || image.includes("/v2/map.png") || thumbnail === "bafkreidj26s7aenyxfthfdibnqonzqm5ptc4iamml744gmcyuokewkr76y" || thumbnail === "bafkreiccptidkodtsbcmk7uw3afmuysu6eboocnr57dmwdg7voylyzg5hy";
  if (placeholder && !(place.world && (place.user_count ?? 0) > 0)) return true;
  const contact = place.contact_name?.trim().toLowerCase();
  return !place.owner && (!contact || contact === "sdk") && (!place.world || (place.user_count ?? 0) === 0);
}

export async function resolveLegacyPlace(kind: "place" | "world", value: string, options: GetOptions = {}): Promise<string | null> {
  if (kind === "world") {
    const name = value.trim().toLowerCase();
    if (!name || name.length > 255 || name.includes(",")) return null;
    const result = await fetchDestinations({ world_names: name, limit: 1 }, options);
    return result.data[0]?.id ?? null;
  }
  if (!/^-?\d+,-?\d+$/.test(value)) return null;
  const positions = value.split(",").map(Number);
  if (!positions.every(Number.isSafeInteger)) return null;
  const raw = await getJSON("/places/api/places", { ...options, query: { positions: positions.join(","), limit: 1 } });
  const result = ApiDataTotalSchema(z.object({ id: z.string() })).parse(raw);
  return result.data[0]?.id ?? null;
}

const WorldRow = WorldRowSchema.pick({
  id: true, title: true, description: true, image: true, owner: true, world_name: true, contact_name: true, base_position: true,
  categories: true, highlighted: true, highlighted_image: true, user_count: true, user_visits: true, favorites: true, likes: true,
  like_rate: true, created_at: true, updated_at: true,
});

/** A world by its name, as decentraland.org's world page reads it (`/api/worlds?names=`); null when no world has it. */
export async function loadWorldPlace(name: string, options: GetOptions = {}): Promise<Place | null> {
  const world = name.trim().toLowerCase();
  if (!world || world.length > 255 || world.includes(",")) return null;
  const raw = await getJSON("/places/api/worlds", { ...options, query: { names: world } });
  const row = ApiDataTotalSchema(z.unknown()).parse(raw).data[0];
  if (row === undefined) return null;
  const parsed = WorldRow.safeParse(row);
  if (!parsed.success) {
    reportSchemaDrift("World", parsed.error.issues);
    return null;
  }
  return { ...parsed.data, positions: [parsed.data.base_position], world: true };
}
