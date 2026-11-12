import type { ReactNode } from "react";
import { Link } from "react-router";

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

import { shortAddress } from "@/data/lib/catalyst/format/address";
import type { ProfileBadge } from "@/data/lib/catalyst/social/badges";
import type { ProfileVM } from "@/data/lib/catalyst/social/profile";

import { useProfileBadges } from "./hooks";

/**
 * A NAME is bought in the Shop's NAMEs page; worlds are managed under Create. Both are the
 * canonical paths, not the redirects (`/shop/items?category=names`, the legacy `/builder/worlds`).
 */
const GET_A_NAME_HREF = "/shop/names";
const MANAGE_WORLD_HREF = "/create/worlds";

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

/** Loads the next tab's code on hover or focus, as the app's other in-app links do. */
function TabLink({ to, ...props }: TabLinkProps) {
  return <Link to={to} prefetch="intent" {...props} />;
}

export type ProfileOverviewScreenProps = {
  profile: ProfileVM;
  /** Whether the wallet has a profile at all; badges are read only then. */
  hasProfile: boolean;
  own: boolean;
  tabs: { id: string; label: string; href: string }[];
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
  const nameColor = profile.nameColor;
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
            addressLabel={shortAddress(profile.address)}
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
      </ProfileLayout>
    </SitesChrome>
  );
}
