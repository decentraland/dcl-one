import { ProfileReferrals } from "@dcl/one-ui/apps/sites/profile/ProfileReferrals";

import { useAuth } from "@/data/lib/auth/index";
import type { ProfileVM } from "@/data/lib/catalyst/social/profile";
import {
  nextReferralTier,
  profileReferralHandle,
  REFERRAL_TIERS,
  referralInviteUrl,
  referralJourneyProgress,
} from "@/data/lib/catalyst/social/referrals";
import { signsFor } from "@/data/lib/profile/queries";

import { useReferralProgress, useReferralTiersSeen } from "./hooks";
import { ProfileShell, type ProfileShellTab } from "./ProfileShell";

export const REFERRAL_TERMS_HREF = "/referral-terms";

const SHARE_TEXT =
  "Wanna hang out in Decentraland? Use my referral link to make your account and download Decentraland so we can explore together! #DCL #Decentraland";

export function shareOnXHref(inviteUrl: string): string {
  return `https://twitter.com/intent/tweet?text=${encodeURIComponent(SHARE_TEXT)}&url=${encodeURIComponent(inviteUrl)}`;
}

export type ProfileReferralsScreenProps = {
  profile: ProfileVM;
  own: boolean;
  tabs: ProfileShellTab[];
  onTab: (id: string) => void;
};

/**
 * The owner's Referral Rewards tab: the invite link, how the program works, and the tiers reached.
 * `own` with no identity yet is the server's render of the owner's page (the wallet cookie names
 * them): it shows loading until the browser has the identity that signs the read.
 */
export function ProfileReferralsScreen({ profile, own, tabs, onTab }: ProfileReferralsScreenProps) {
  const { identity } = useAuth();
  const signed = signsFor(identity, profile.address);
  const query = useReferralProgress(profile.address, identity);
  const tiersSeen = useReferralTiersSeen(profile.address, identity);

  const inviteUrl = referralInviteUrl(
    typeof window === "undefined" ? "" : window.location.origin,
    profileReferralHandle(profile),
  );
  const progress = query.data;
  const journey = progress
    ? {
        ...progress,
        tiers: REFERRAL_TIERS,
        goal: REFERRAL_TIERS[REFERRAL_TIERS.length - 1].invites,
        remaining: (nextReferralTier(progress.accepted)?.invites ?? progress.accepted) - progress.accepted,
        fraction: referralJourneyProgress(progress.accepted),
      }
    : undefined;

  return (
    <ProfileShell profile={profile} own={own} tabs={tabs} active="referral-rewards" onTab={onTab}>
      <ProfileReferrals
        journey={journey}
        profileAddress={profile.address}
        inviteUrl={inviteUrl}
        shareHref={shareOnXHref(inviteUrl)}
        termsHref={REFERRAL_TERMS_HREF}
        onShown={tiersSeen}
        requireSignIn={!signed && !own}
        failed={query.isError}
        retrying={query.isFetching}
        onRetry={() => void query.refetch()}
      />
    </ProfileShell>
  );
}
