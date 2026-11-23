import { z } from "zod";

import { CatalystError, getJSON } from "../client";
import type { GetOptions } from "../client";
import { ETH_ADDRESS_RE } from "../format/address";

const AvatarSchema = z.object({
  userId: z.string().optional(),
  name: z.string().optional(),
  ethAddress: z.string().optional(),
  hasClaimedName: z.boolean().optional(),
  avatar: z.object({ snapshots: z.object({ face256: z.string().nullish() }).nullish() }).nullish(),
});

const ProfileLambdaSchema = z.object({
  avatars: z.array(AvatarSchema),
});
type ProfileLambda = z.infer<typeof ProfileLambdaSchema>;

const ReferralTierSchema = z.object({
  tier: z.number(),
  invitesAccepted: z.number(),
  rarity: z.string(),
  description: z.string(),
});

export const ReferralStateSchema = z.object({
  invitedUsers: z.number(),
  invitedUsersAccepted: z.number(),
  currentTier: z.number(),
  nextTier: z.number(),
  tiers: z.array(ReferralTierSchema),
});
export type ReferralState = z.infer<typeof ReferralStateSchema>;

export type ReferrerVM = {
  /** The inviter's display name, as decentraland.org shows it: the profile's first avatar name, trimmed. */
  name: string | null;
  address: string | null;
  hasClaimedName: boolean;
  resolved: boolean;
  /** The avatar's 256px face snapshot as the profile names it; the invite's card resolves it through `faceUrl`. */
  face256: string | null;
};

function toReferrerVM(profile: ProfileLambda, address: string): ReferrerVM {
  const a = profile.avatars[0];
  const name = a?.name?.trim() || null;
  const face = a?.avatar?.snapshots?.face256?.trim();
  return {
    name,
    address,
    hasClaimedName: Boolean(a?.hasClaimedName && name),
    resolved: Boolean(a),
    face256: face || null,
  };
}

export function referrerFromAddress(address: string | null): ReferrerVM {
  return { name: null, address, hasClaimedName: false, resolved: false, face256: null };
}

/** The wallet an invite names: the address itself, or the owner of the NAME. None when the NAME has no owner; a lookup that fails throws. */
export async function resolveReferrerAddress(referrer: string, opts: GetOptions = {}): Promise<string | null> {
  const p = referrer.trim();
  if (ETH_ADDRESS_RE.test(p)) return p.toLowerCase();
  if (!p) return null;
  const raw = await getJSON<{ owner?: unknown } | null>(`/lambdas/names/${encodeURIComponent(p)}/owner`, opts).catch((err) => {
    if (err instanceof CatalystError && err.status === 404) return null;
    throw err;
  });
  return raw?.owner ? String(raw.owner).toLowerCase() : null;
}

export async function fetchReferrerProfile(
  address: string,
  opts: GetOptions = {},
): Promise<ReferrerVM> {
  const lower = address.toLowerCase();
  const raw = await getJSON<unknown>(`/lambdas/profiles/${encodeURIComponent(lower)}`, opts);
  return toReferrerVM(ProfileLambdaSchema.parse(raw), lower);
}
