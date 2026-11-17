import { queryOptions } from "@tanstack/react-query";
import { z } from "zod";

import { profileFaceUrl } from "../../auth/profile-face-url";
import { catalystBase } from "../client";

/**
 * Creator by-lines: the name and face of a batch of wallets, read with one `POST /lambdas/profiles`.
 * Client-safe. The same shape and batching as the /places discovery feed's creator read.
 */

/** A profile's name and face change rarely; the by-lines of a whole page share one read. */
export const CREATOR_PROFILES_STALE_MS = 5 * 60_000;

export type CreatorProfile = { name: string; avatarUrl?: string };
export type CreatorProfiles = Record<string, CreatorProfile>;

const ADDRESS = /^0x[0-9a-f]{40}$/;

const ProfilesEnvelope = z.array(
  z
    .object({
      avatars: z.array(
        z
          .object({
            ethAddress: z.string(),
            name: z.string().optional(),
            avatar: z
              .object({ snapshots: z.object({ face256: z.string().optional() }).passthrough().optional() })
              .passthrough()
              .optional(),
          })
          .passthrough(),
      ),
    })
    .passthrough(),
);

/** One read for a batch of wallets; a wallet without a profile is left out. */
export async function fetchCreatorProfiles(
  addresses: readonly string[],
  signal?: AbortSignal,
): Promise<CreatorProfiles> {
  if (!addresses.length) return {};
  const res = await fetch(`${catalystBase()}/lambdas/profiles`, {
    method: "POST",
    headers: { accept: "application/json", "content-type": "application/json" },
    body: JSON.stringify({ ids: addresses }),
    signal,
  });
  if (!res.ok) {
    // Release the connection before failing: an unread body keeps it busy.
    await res.body?.cancel().catch(() => undefined);
    throw new Error(`Profiles returned ${res.status}`);
  }
  const out: CreatorProfiles = {};
  for (const entry of ProfilesEnvelope.parse(await res.json())) {
    const avatar = entry.avatars[0];
    if (!avatar) continue;
    const face = profileFaceUrl(avatar.avatar?.snapshots?.face256);
    out[avatar.ethAddress.toLowerCase()] = { name: avatar.name ?? "", ...(face ? { avatarUrl: face } : {}) };
  }
  return out;
}

export const creatorProfilesKeys = {
  batch: (addresses: readonly string[]) => ["creators", ...addresses] as const,
};

/** Sorted and deduplicated so the same creators share one entry whatever order the cards come in. */
export const creatorProfilesQuery = (addresses: readonly string[]) => {
  const ids = [...new Set(addresses.map((address) => address.toLowerCase()))].sort();
  return queryOptions({
    queryKey: creatorProfilesKeys.batch(ids),
    staleTime: CREATOR_PROFILES_STALE_MS,
    queryFn: ({ signal }) => fetchCreatorProfiles(ids, signal),
  });
};

/**
 * One batch per group of cards (a rail, a list), each holding only the wallets no earlier group asked
 * for, so a group that loads later reads only its own new creators. Values that are not wallets are
 * dropped.
 */
export function creatorBatches(groups: readonly (readonly (string | null | undefined)[])[]): string[][] {
  const asked = new Set<string>();
  const batches: string[][] = [];
  for (const group of groups) {
    const fresh: string[] = [];
    for (const value of group) {
      const address = value?.trim().toLowerCase();
      if (!address || !ADDRESS.test(address) || asked.has(address)) continue;
      asked.add(address);
      fresh.push(address);
    }
    if (fresh.length) batches.push(fresh);
  }
  return batches;
}
