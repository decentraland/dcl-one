import { z } from "zod";

import { profileFaceUrl } from "../../auth/profile-face-url";
import { catalystBase, postPublicJSON, type GetOptions } from "../client";
import { ETH_ADDRESS_RE } from "../format/address";

/**
 * The name, face and claimed-name flag of a batch of wallets, read from `POST /lambdas/profiles`
 * 100 wallets at a time. Client-safe.
 */

/** A profile's name and face change rarely. */
export const MEMBER_PROFILES_STALE_MS = 5 * 60_000;

export type MemberProfile = { name: string; picture: string | null; claimed: boolean | null };
export type MemberProfiles = Record<string, MemberProfile>;

const BATCH = 100;

/** The wallet a value names, lowercased; null for anything that is not a wallet. */
export function asAddress(raw: unknown): string | null {
  if (typeof raw !== "string") return null;
  const value = raw.trim();
  return ETH_ADDRESS_RE.test(value) ? value.toLowerCase() : null;
}

/**
 * Re-base a content-hash URL on the catalyst host, or an upstream face snapshot, onto the node this
 * page reads: a cross-origin `<img>` of the catalyst host's `application/octet-stream` answer is
 * refused by Chrome's ORB, and the upstream profile-images CDN 404s for profiles it never rendered.
 */
export function sameOriginContentUrl(url: string, base = catalystBase()): string {
  const hash = /^https?:\/\/[^/]+\/content\/contents\/([A-Za-z0-9]+)\/?$/.exec(url)?.[1];
  if (hash) return `${base}/content/contents/${hash}`;
  return /^https?:\/\//i.test(url) ? profileFaceUrl(url, base) || url : url;
}

const Envelope = z
  .object({
    avatars: z.array(
      z
        .object({
          ethAddress: z.string().optional(),
          userId: z.string().optional(),
          name: z.string().nullish(),
          hasClaimedName: z.boolean().nullish(),
          avatar: z
            .object({ snapshots: z.object({ face256: z.string().optional() }).passthrough().nullish() })
            .passthrough()
            .optional(),
        })
        .passthrough(),
    ),
  })
  .passthrough();

type Avatar = z.infer<typeof Envelope>["avatars"][number];

/** A name that is a wallet is no name. */
export const memberName = (name: string) => (asAddress(name) ? "" : name.trim());

function faceUrl(avatar: Avatar, base: string): string | null {
  const snap = avatar.avatar?.snapshots?.face256;
  if (!snap) return null;
  if (/^https?:\/\//i.test(snap) || snap.startsWith("data:")) return sameOriginContentUrl(snap, base);
  return `${base}/content/contents/${snap}`;
}

/** One entry per wallet that has a profile; a wallet without one is left out. */
export async function fetchMemberProfiles(
  addresses: readonly string[],
  opts: Pick<GetOptions, "base" | "signal" | "fetchImpl"> = {},
): Promise<MemberProfiles> {
  const base = catalystBase(opts.base ?? (import.meta.env.VITE_CATALYST_URL || undefined));
  const profiles: MemberProfiles = {};
  for (let i = 0; i < addresses.length; i += BATCH) {
    const batch = await postPublicJSON(
      "/lambdas/profiles",
      { ids: addresses.slice(i, i + BATCH) },
      { ...opts, base, allowEmpty: true },
    );
    for (const envelope of Array.isArray(batch) ? batch : []) {
      // One unreadable profile must not cost the rest of its batch their names.
      const parsed = Envelope.safeParse(envelope);
      if (!parsed.success) continue;
      for (const avatar of parsed.data.avatars) {
        const address = asAddress(avatar.ethAddress) ?? asAddress(avatar.userId);
        if (address) {
          profiles[address] = {
            name: memberName(avatar.name ?? ""),
            picture: faceUrl(avatar, base),
            claimed: avatar.hasClaimedName ?? null,
          };
        }
      }
    }
  }
  return profiles;
}
