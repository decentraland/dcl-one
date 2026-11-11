export type { Place, Category } from "../schema";

import { z } from "zod";

import { getJSON } from "../client";
import type { GetOptions } from "../client";
import { placesApiPath } from "../typed";
import { ApiDataTotalSchema } from "../generated-schemas/places";
import { parsePlaces, reportSchemaDrift } from "../schema";
import type { Place } from "../schema";

export type PlaceCardProps = {
  id: string;
  title: string;
  image?: string;
  players: number | null;
  rating: number;
  coords: string;
  live: boolean;
  featured: boolean;
  creator: string;
};

export function mapPlace(p: Place): PlaceCardProps {
  const players = p.user_count ?? 0;
  return {
    id: p.id,
    title: p.title ?? "",
    image: p.image ?? undefined,
    players: p.user_count,
    rating: Math.round((p.like_rate ?? 0) * 100),
    coords: p.world ? p.world_name ?? "" : p.base_position,
    live: players > 0,
    featured: p.highlighted,
    creator: p.contact_name ?? p.owner ?? "",
  };
}

const PlacesListEnvelope = ApiDataTotalSchema(z.unknown());

export async function fetchMostActivePlaces(
  params: { limit?: number } = {},
  opts: GetOptions = {},
): Promise<Place[]> {
  const raw = await getJSON<unknown>(placesApiPath("get", "/api/places"), {
    ...opts,
    query: {
      limit: params.limit ?? 4,
      order_by: "most_active",
      with_realtime_active_users: true,
    },
  });
  const env = PlacesListEnvelope.safeParse(raw);
  if (!env.success) {
    reportSchemaDrift("MostActivePlacesEnvelope", env.error.issues);
    const loose = z.object({ data: z.array(z.unknown()) }).safeParse(raw);
    return parsePlaces(loose.success ? loose.data.data : []);
  }
  return parsePlaces(env.data.data);
}
