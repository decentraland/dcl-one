import type { ReactNode } from "react";
import { Link } from "react-router";

import SitesLayout from "../components/chrome/SitesLayout";

import { Button, type TabsLinkProps } from "@dcl/one-ui";
import { ProfileFrame, type ProfileShellTab, ProfileHeader } from "@dcl/one-ui/apps/sites";


import { SHOP_BASE } from "@/core/lib/shop/redirects";
import { useAuth } from "@/data/lib/auth/index";
import { shortAddress } from "@/data/lib/catalyst/format/address";
import type { ProfileVM } from "@/data/lib/catalyst/social/profile";

import { ProfileMemberActions } from "./ProfileMemberActions";
import { ProfileFriendsCount, ProfileInviteFriends } from "./ProfileOwnerActions";

/**
 * A NAME is bought in the Shop's NAMEs page (the canonical path, not `/shop/items?category=names`);
 * worlds are managed in the Creator Hub desktop app.
 */
export const GET_A_NAME_HREF = `${SHOP_BASE}/names`;
export const MANAGE_WORLD_HREF = "/download/creator-hub";

export type { ProfileShellTab };

/** Loads the next tab's code on hover or focus, as the app's other in-app links do. */
function TabLink({ href, ...props }: TabsLinkProps) {
  return <Link to={href} prefetch="intent" {...props} />;
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

/**
 * The profile site's frame every ported tab shares: Sites chrome, header with the owner's call to
 * action or another member's friendship, block and share actions, tab bar.
 */
export function ProfileShell({ profile, own, tabs, active, onTab, aside, children }: ProfileShellProps) {
  const { identity } = useAuth();
  const actions = own ? (
    <>
      {profile.hasClaimedName ? (
        <Button href={MANAGE_WORLD_HREF} size="md">
          Manage World
        </Button>
      ) : (
        <Button href={GET_A_NAME_HREF} size="md">
          Get a Unique Name
        </Button>
      )}
      <ProfileFriendsCount address={profile.address} />
      <ProfileInviteFriends profile={profile} />
    </>
  ) : (
    // Keyed by address: another profile starts with no result of the last one's actions.
    <ProfileMemberActions
      key={`${identity?.signer ?? ""}:${profile.address}`}
      address={profile.address}
      name={profile.name}
    />
  );

  // No top-bar entry is highlighted on a profile ("play" maps to none), as on the profile site.
  return (
    <SitesLayout active="play">
      <ProfileFrame
        tabs={tabs}
        active={active}
        onTab={onTab}
        linkComponent={TabLink}
        aside={aside}
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
      >
        {children}
      </ProfileFrame>
    </SitesLayout>
  );
}
