import type { AuthIdentity } from "../../auth/types";
import { signedGetJSON } from "../client";
import { ReferralProgressStatsSchema } from "../generated-schemas/communities";

const PATH = "/v1/referral-progress";

export type ReferralProgress = {
  /** Invited friends who count toward the rewards. */
  accepted: number;
  /** How many of them the member had already seen on an earlier visit. */
  viewed: number;
  /** The granted rewards' own pictures, by the tier's invite count; https only. */
  rewardImages: Record<number, string>;
};

/**
 * The signed-in member's referral progress. The signed request names the account, so there is no
 * address in the path; the node also marks the count as seen on this read.
 */
export async function fetchReferralProgress(
  identity: AuthIdentity,
  opts: { signal?: AbortSignal } = {},
): Promise<ReferralProgress> {
  const raw = ReferralProgressStatsSchema.parse(await signedGetJSON<unknown>(PATH, { identity, signal: opts.signal }));
  const rewardImages: Record<number, string> = {};
  for (const image of raw.rewardImages) {
    if (/^https:\/\//i.test(image.url)) rewardImages[image.tier] = image.url;
  }
  return { accepted: raw.invitedUsersAccepted, viewed: raw.invitedUsersAcceptedViewed, rewardImages };
}

export type ReferralRarity = "epic" | "legendary" | "exotic" | "mythic" | "swag";

export type ReferralTier = {
  /** Invited friends the tier needs; also the key of its picture in `rewardImages`. */
  invites: number;
  rarity: ReferralRarity;
  reward: string;
};

/** The reward program's tiers, in order. */
export const REFERRAL_TIERS: readonly ReferralTier[] = [
  { invites: 5, rarity: "epic", reward: "RainbowWave Jeans + Community Recruiter Starter Badge" },
  { invites: 10, rarity: "epic", reward: "Good Vibes Letterman + Community Recruiter Bronze Badge" },
  { invites: 20, rarity: "legendary", reward: "Signal Surge Gloves" },
  { invites: 25, rarity: "legendary", reward: "Postman Emote + Community Recruiter Silver Badge" },
  { invites: 30, rarity: "exotic", reward: "Afterglow Kicks" },
  { invites: 50, rarity: "exotic", reward: "Spunky MopTop + Community Recruiter Gold Badge" },
  { invites: 60, rarity: "mythic", reward: "Volty Vibes Shoulder Companion" },
  { invites: 75, rarity: "mythic", reward: "Monocycle Emote + Community Recruiter Platinum Badge" },
  { invites: 100, rarity: "swag", reward: "IRL Swag Pack + In-World Wearable + Community Recruiter Diamond Badge" },
];

/** The first tier the member has not reached, or `null` after the last one. */
export function nextReferralTier(accepted: number): ReferralTier | null {
  return REFERRAL_TIERS.find((tier) => accepted < tier.invites) ?? null;
}

/**
 * How far along the journey's track the member is, from 0 to 1. The tiers sit at the centre of
 * equal columns, so tier `i` of `n` is at `(i + 0.5) / n`; between two tiers the track fills in
 * proportion to the invites between them, and it is full from the last tier on.
 */
export function referralJourneyProgress(accepted: number): number {
  const count = REFERRAL_TIERS.length;
  const last = REFERRAL_TIERS[count - 1];
  if (accepted <= 0) return 0;
  if (accepted >= last.invites) return 1;
  const next = REFERRAL_TIERS.findIndex((tier) => accepted < tier.invites);
  const fromInvites = next === 0 ? 0 : REFERRAL_TIERS[next - 1].invites;
  const fromPosition = next === 0 ? 0 : (next - 0.5) / count;
  const toPosition = (next + 0.5) / count;
  const share = (accepted - fromInvites) / (REFERRAL_TIERS[next].invites - fromInvites);
  return fromPosition + share * (toPosition - fromPosition);
}

/** The same referral link is shared by the Profile header and the Referral Rewards tab. */
export function profileReferralHandle(profile: { address: string; name: string; hasClaimedName: boolean }): string {
  return profile.hasClaimedName && profile.name ? profile.name : profile.address;
}

/** The invite page a member shares: by claimed NAME when they have one, by wallet otherwise. */
export function referralInviteUrl(origin: string, handle: string): string {
  return `${origin.replace(/\/+$/, "")}/invite/${encodeURIComponent(handle)}`;
}
