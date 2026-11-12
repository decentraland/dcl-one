import type { ReactNode } from "react";
import { Link } from "react-router";

import SitesChrome from "@ui/web/frames/SitesChrome";

import { Button } from "@dcl/one-ui/apps/sites/shared/Button/Button";
import { ProfileHeader } from "@dcl/one-ui/apps/sites/provisional/profile/ProfileHeader";
import { ProfileLayout } from "@dcl/one-ui/apps/sites/provisional/profile/ProfileLayout";
import { ProfileTabs, type TabLinkProps } from "@dcl/one-ui/apps/sites/provisional/profile/ProfileTabs";

import { shortAddress } from "@/data/lib/catalyst/format/address";
import type { ProfileVM } from "@/data/lib/catalyst/social/profile";

/**
 * A NAME is bought in the Shop's NAMEs page; worlds are managed under Create. Both are the
 * canonical paths, not the redirects (`/shop/items?category=names`, the legacy `/builder/worlds`).
 */
export const GET_A_NAME_HREF = "/shop/names";
export const MANAGE_WORLD_HREF = "/create/worlds";

export type ProfileShellTab = { id: string; label: string; href: string };

/** Loads the next tab's code on hover or focus, as the app's other in-app links do. */
function TabLink({ to, ...props }: TabLinkProps) {
  return <Link to={to} prefetch="intent" {...props} />;
}

type ProfileShellProps = {
  profile: ProfileVM;
  own: boolean;
  tabs: ProfileShellTab[];
  active: string;
  onTab: (id: string) => void;
  /** The avatar column; only Overview has one. */
  aside?: ReactNode;
  children: ReactNode;
};

/** The profile site's frame every ported tab shares: Sites chrome, header with the owner's call to action, tab bar. */
export function ProfileShell({ profile, own, tabs, active, onTab, aside, children }: ProfileShellProps) {
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
            nameColor={profile.nameColor}
            hasClaimedName={profile.hasClaimedName}
            avatarUrl={profile.avatarUrl || undefined}
            actions={actions}
          />
        }
        tabs={<ProfileTabs tabs={tabs} active={active} onSelect={onTab} LinkComponent={TabLink} />}
        aside={aside}
      >
        {children}
      </ProfileLayout>
    </SitesChrome>
  );
}
