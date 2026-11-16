import { z } from "zod";

import { getJSON } from "../client";
import type { GetOptions } from "../client";

const RawBadgeSchema = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string().nullish(),
  /** Epoch milliseconds as a string, as the badges service ships it. */
  completedAt: z.string().nullish(),
  assets: z.object({ "2d": z.object({ normal: z.string().nullish() }).partial().nullish() }).partial().nullish(),
  progress: z
    .object({
      lastCompletedTierAt: z.union([z.number(), z.string()]).nullish(),
      lastCompletedTierName: z.string().nullish(),
      lastCompletedTierImage: z.string().nullish(),
    })
    .partial()
    .nullish(),
});

/** The node always sends `achieved`; a reply without it is broken, not a member with no badges. */
const UserBadgesSchema = z.object({ data: z.object({ achieved: z.array(RawBadgeSchema) }) });

export type ProfileBadge = {
  id: string;
  name: string;
  imageUrl?: string;
  /** The tier the member has reached, for a tiered badge. */
  tierName?: string;
  /** The reached tier's blurb for a tiered badge, else the badge's own description. */
  description?: string;
  /** When the badge (or its latest tier) was unlocked, epoch milliseconds. */
  unlockedAt?: number;
};

/**
 * A tiered badge lists every tier's blurb in one description (`Bronze: 50 scenes;Silver: 250 …`);
 * only the reached tier's is shown.
 */
export function tierDescription(description: string | undefined, tierName: string | undefined): string | undefined {
  if (!description || !tierName) return description || undefined;
  const target = `${tierName.toLowerCase()}:`;
  const chunk = description
    .split(";")
    .map((part) => part.trim())
    .find((part) => part.toLowerCase().startsWith(target));
  return chunk ? chunk.replace(/^[^:]+:\s*/, "").trim() || description : description;
}

function toMillis(value: number | string | null | undefined): number | undefined {
  if (value === null || value === undefined || value === "") return undefined;
  const ms = typeof value === "string" ? Number(value) : value;
  return Number.isFinite(ms) ? ms : undefined;
}

/** The badges a member has achieved, from the node's `/badges` mount. */
export async function fetchUserBadges(address: string, opts: GetOptions = {}): Promise<ProfileBadge[]> {
  const raw = await getJSON<unknown>(`/badges/users/${encodeURIComponent(address.toLowerCase())}/badges`, opts);
  return UserBadgesSchema.parse(raw).data.achieved.map((badge) => {
    const tierName = badge.progress?.lastCompletedTierName ?? undefined;
    return {
      id: badge.id,
      name: badge.name,
      imageUrl: badge.progress?.lastCompletedTierImage ?? badge.assets?.["2d"]?.normal ?? undefined,
      tierName,
      description: tierDescription(badge.description ?? undefined, tierName),
      unlockedAt: toMillis(badge.progress?.lastCompletedTierAt) ?? toMillis(badge.completedAt),
    };
  });
}
