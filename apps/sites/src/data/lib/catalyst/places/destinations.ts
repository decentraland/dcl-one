import { z } from "zod";
import { readCopy, type CopyStamp } from "@ui/data/copyAsOf";
import type { AuthIdentity } from "../../auth/types";
import { getJSON, signedGetJSON, type GetOptions } from "../client";
import { ApiDataTotalSchema, DestinationSchema } from "../generated-schemas/places";
import type { Place } from "../schema";

export type DestinationParams = {
  limit?: number; offset?: number; categories?: string; search?: string; owner?: string;
  only_favorites?: boolean; only_highlighted?: boolean; world_names?: string;
  order_by?: "most_active";
};
export type DestinationPage = { data: Place[]; total: number; copy: CopyStamp | null };

function toPlace(row: z.infer<typeof DestinationSchema>): Place {
  return {
    id: row.id, title: row.title, description: row.description, image: row.image, owner: row.owner,
    positions: row.positions, base_position: row.base_position, updated_at: row.updated_at, created_at: row.created_at,
    contact_name: row.contact_name, categories: row.categories, highlighted: row.highlighted, highlighted_image: row.highlighted_image,
    user_count: row.user_count, user_visits: row.user_visits, favorites: row.favorites, likes: row.likes, like_rate: row.like_rate,
    world: row.world, world_name: row.world_name,
  };
}

export async function fetchDestinations(params: DestinationParams = {}, options: GetOptions & { identity?: AuthIdentity } = {}): Promise<DestinationPage> {
  if (params.only_favorites && !options.identity) throw new Error("Sign in to see your favourites.");
  const query = { order_by: "most_active", ...params, with_realms_detail: true, with_live_events: true };
  const path = "/places/api/destinations";
  const raw = options.identity
    ? await signedGetJSON(path, { identity: options.identity, base: options.base, signal: options.signal, query })
    : await getJSON(path, { ...options, query });
  const envelope = ApiDataTotalSchema(DestinationSchema).parse(raw);
  return { data: envelope.data.map(toPlace), total: envelope.total, copy: readCopy(raw) };
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
    if (!name || name.length > 255) return null;
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
