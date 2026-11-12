// What the Overview tab needs beside the profile: the badges and, for a visitor's view, which data tabs
// have a first row. The probes ask for one row each, as decentraland.org's profile does
// (sites: src/components/profile/ProfileTabs/useProfileTabAvailability.ts).
import { z } from "zod";

import { getJSON, type GetOptions } from "../catalyst/client";
import { normalizeAddress } from "../catalyst/social/profile";
import { placesApiPath } from "../catalyst/typed";

export type ProfileBadge = { id: string; name: string; image: string | null };

const BadgeSchema = z
  .object({
    id: z.string(),
    name: z.string(),
    assets: z
      .object({ "2d": z.object({ normal: z.string().optional() }).passthrough().optional() })
      .passthrough()
      .optional(),
    progress: z.object({ lastCompletedTierImage: z.string().nullish() }).passthrough().optional(),
  })
  .passthrough();

const BadgesEnvelopeSchema = z
  .object({ data: z.object({ achieved: z.array(BadgeSchema) }).passthrough() })
  .passthrough();

/** The badges the address has earned (the node's badges service, `/badges/users/{address}/badges`). */
export async function fetchProfileBadges(
  address: string,
  opts: GetOptions = {},
): Promise<ProfileBadge[]> {
  const raw = await getJSON<unknown>(
    `/badges/users/${encodeURIComponent(normalizeAddress(address))}/badges`,
    opts,
  );
  return BadgesEnvelopeSchema.parse(raw).data.achieved.map((badge) => ({
    id: badge.id,
    name: badge.name,
    image: badge.progress?.lastCompletedTierImage || badge.assets?.["2d"]?.normal || null,
  }));
}

export type DataTab = "places" | "photos" | "creations" | "communities";

const TotalSchema = z
  .object({ total: z.number().optional(), data: z.array(z.unknown()).optional() })
  .passthrough();

function total(raw: unknown): number {
  const parsed = TotalSchema.safeParse(raw);
  if (!parsed.success) return 0;
  return parsed.data.total ?? parsed.data.data?.length ?? 0;
}

const PROBE = { limit: 1, offset: 0 } as const;

/**
 * The member tabs that have no row yet, so a visitor's view hides them. A probe that fails keeps its
 * tab hidden, as decentraland.org does.
 */
export async function probeEmptyTabs(address: string, opts: GetOptions = {}): Promise<DataTab[]> {
  const owner = normalizeAddress(address);
  const read = (path: string, query: Record<string, string | number>) =>
    getJSON<unknown>(path, { ...opts, query }).catch(() => null);
  const catalog = (category: string) =>
    read("/marketplace-api/v2/catalog", {
      category,
      creator: owner,
      first: 1,
      skip: 0,
      sortBy: "newest",
    });
  const [places, worlds, wearables, emotes, photos, communities] = await Promise.all([
    read(placesApiPath("get", "/api/places"), { ...PROBE, owner }),
    read(placesApiPath("get", "/api/worlds"), { ...PROBE, owner }),
    catalog("wearable"),
    catalog("emote"),
    read(`/camera-reel-service/api/users/${encodeURIComponent(owner)}/images`, PROBE),
    read(`/social-api/v1/members/${encodeURIComponent(owner)}/communities`, PROBE),
  ]);
  const photoTotal = z.object({ images: z.array(z.unknown()) }).passthrough().safeParse(photos);
  const communityTotal = z.object({ data: TotalSchema }).passthrough().safeParse(communities);
  const counts: Record<DataTab, number> = {
    places: total(places) + total(worlds),
    creations: wearables === null || emotes === null ? 0 : total(wearables) + total(emotes),
    photos: photoTotal.success ? photoTotal.data.images.length : 0,
    communities: communityTotal.success ? total(communityTotal.data.data) : 0,
  };
  return (Object.keys(counts) as DataTab[]).filter((tab) => counts[tab] === 0);
}
