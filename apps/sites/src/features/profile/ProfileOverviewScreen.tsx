import type { ReactNode } from "react";

import {
  ProfileCountry,
  ProfileGender,
  ProfileHobbies,
  ProfileLanguage,
  ProfileProfession,
  ProfilePronouns,
  ProfileRealName,
  ProfileRelationship,
} from "@dcl/one-ui/foundations/icons";
import { ProfileAvatarBody } from "@dcl/one-ui/apps/sites/provisional/profile/ProfileAvatarBody";
import { ProfileBadges } from "@dcl/one-ui/apps/sites/provisional/profile/ProfileBadges";
import { ProfileOverview } from "@dcl/one-ui/apps/sites/provisional/profile/ProfileOverview";

import type { ProfileBadge } from "@/data/lib/catalyst/social/badges";
import type { ProfileVM } from "@/data/lib/catalyst/social/profile";

import { useProfileBadges } from "./hooks";
import { ProfileShell, type ProfileShellTab } from "./ProfileShell";


const INFO_ICONS: Record<string, ReactNode> = {
  country: <ProfileCountry size={20} />,
  language: <ProfileLanguage size={20} />,
  pronouns: <ProfilePronouns size={20} />,
  relationship_status: <ProfileRelationship size={20} />,
  gender: <ProfileGender size={20} />,
  profession: <ProfileProfession size={20} />,
  real_name: <ProfileRealName size={20} />,
  favorite_hobby: <ProfileHobbies size={20} />,
};

export type ProfileOverviewScreenProps = {
  profile: ProfileVM;
  /** Whether the wallet has a profile at all; badges are read only then. */
  hasProfile: boolean;
  own: boolean;
  tabs: ProfileShellTab[];
  onTab: (id: string) => void;
};

/** The site is in English; a fixed locale keeps the server and browser renders the same. */
const UNLOCKED = new Intl.DateTimeFormat("en-US", { year: "numeric", month: "short", day: "numeric", timeZone: "UTC" });

export function badgeItems(badges: readonly ProfileBadge[]) {
  return badges.map(({ unlockedAt, ...badge }) => ({
    ...badge,
    unlocked: unlockedAt === undefined ? undefined : `Unlocked on ${UNLOCKED.format(unlockedAt)}`,
  }));
}

/** Overview with the profile site's look: header, tab bar, avatar column, About and Links. */
export function ProfileOverviewScreen({
  profile,
  hasProfile,
  own,
  tabs,
  onTab,
}: ProfileOverviewScreenProps) {
  const badges = useProfileBadges(profile.address, hasProfile);

  return (
    <ProfileShell
      profile={profile}
      own={own}
      tabs={tabs}
      active="overview"
      onTab={onTab}
      aside={<ProfileAvatarBody imageUrl={profile.bodyUrl || undefined} name={profile.name} />}
    >
      <ProfileOverview
        badges={
          <ProfileBadges
            badges={badgeItems(badges.data ?? [])}
            loading={hasProfile && badges.isPending}
            emptyText="No badges yet. Jump in world and start collecting badges!"
            // A failed background re-read keeps the badges already shown.
            errorText={badges.isError && !badges.data ? "Badges are unavailable right now." : undefined}
          />
        }
        bio={profile.bio}
        emptyBio={
          own
            ? "No information yet. Jump in world and complete your bio!"
            : "This member has not added a bio yet."
        }
        info={profile.info.map((item) => ({
          ...item,
          icon: INFO_ICONS[item.key],
        }))}
        links={profile.links}
      />
    </ProfileShell>
  );
}
