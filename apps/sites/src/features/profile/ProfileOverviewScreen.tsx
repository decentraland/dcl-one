import type { ReactNode } from "react";
import { Link } from "react-router";

import { getAvatarBackgroundColor, getDisplayName } from "@ui/shop/discovery/lib/avatarColor";
import SitesChrome from "@ui/web/frames/SitesChrome";

import { Button } from "@dcl/one-ui/apps/sites/shared/Button/Button";
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
import { ProfileHeader } from "@dcl/one-ui/apps/sites/provisional/profile/ProfileHeader";
import { ProfileLayout } from "@dcl/one-ui/apps/sites/provisional/profile/ProfileLayout";
import { ProfileOverview } from "@dcl/one-ui/apps/sites/provisional/profile/ProfileOverview";
import {
  ProfileTabs,
  type TabLinkProps,
} from "@dcl/one-ui/apps/sites/provisional/profile/ProfileTabs";

import type { ProfileBadge } from "@/data/lib/catalyst/social/badges";
import type { ProfileVM } from "@/data/lib/catalyst/social/profile";

import { useProfileBadges } from "./hooks";

/** A NAME is bought in the Shop; worlds are managed in the Builder, both served by path. */
export const GET_A_NAME_HREF = "/shop/items?category=names";
export const MANAGE_WORLD_HREF = "/builder/worlds";

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

function TabLink({ to, ...props }: TabLinkProps) {
  return <Link to={to} {...props} />;
}

export type ProfileOverviewScreenProps = {
  profile: ProfileVM;
  /** Whether the wallet has a profile at all; badges are read only then. */
  hasProfile: boolean;
  own: boolean;
  tabs: { id: string; label: string; href: string }[];
  onTab: (id: string) => void;
};

/** Overview with the profile site's look: header, tab bar, avatar column, About and Links. */
/** The site is in English; a fixed locale keeps the server and browser renders the same. */
const UNLOCKED = new Intl.DateTimeFormat("en-US", { year: "numeric", month: "short", day: "numeric", timeZone: "UTC" });

export function badgeItems(badges: readonly ProfileBadge[]) {
  return badges.map(({ unlockedAt, ...badge }) => ({
    ...badge,
    unlocked: unlockedAt === undefined ? undefined : `Unlocked on ${UNLOCKED.format(unlockedAt)}`,
  }));
}

/**
 * The name and avatar color every Decentraland client derives from the display name (the
 * explorer's NameColorHelper), not the profile's stored color.
 */
export function profileNameColor(profile: ProfileVM): string {
  return getAvatarBackgroundColor(getDisplayName(profile.name, profile.hasClaimedName, profile.address));
}

export function ProfileOverviewScreen({
  profile,
  hasProfile,
  own,
  tabs,
  onTab,
}: ProfileOverviewScreenProps) {
  const nameColor = profileNameColor(profile);
  const badges = useProfileBadges(profile.address, hasProfile);
  const actions = own ? (
    profile.hasClaimedName ? (
      <Button href={MANAGE_WORLD_HREF}>Manage World</Button>
    ) : (
      <Button href={GET_A_NAME_HREF}>Get a Unique Name</Button>
    )
  ) : null;

  // No top-bar entry is highlighted on a profile ("play" maps to none), as on the profile site.
  return (
    <SitesChrome active="play" overlayNav>
      <ProfileLayout
        header={
          <ProfileHeader
            address={profile.address}
            name={profile.name}
            nameColor={nameColor}
            hasClaimedName={profile.hasClaimedName}
            avatarUrl={profile.avatarUrl || undefined}
            actions={actions}
          />
        }
        tabs={
          <ProfileTabs
            tabs={tabs}
            active="overview"
            onSelect={onTab}
            LinkComponent={TabLink}
          />
        }
        aside={
          <ProfileAvatarBody
            imageUrl={profile.bodyUrl || undefined}
            name={profile.name}
          />
        }
      >
        <ProfileOverview
          badges={
            <ProfileBadges
              badges={badgeItems(badges.data ?? [])}
              loading={hasProfile && badges.isPending}
              emptyText="No badges yet. Jump in world and start collecting badges!"
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
      </ProfileLayout>
    </SitesChrome>
  );
}
