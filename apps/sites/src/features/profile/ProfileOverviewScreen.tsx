import { useEffect, useRef, useState, type ReactNode } from "react";
import { Link } from "react-router";

import { getAvatarBackgroundColor, getDisplayName } from "@ui/shop/discovery/lib/avatarColor";
import SitesChrome from "@ui/web/frames/SitesChrome";

import {
  Icon,
  PronounsIcon,
  type IconName,
} from "@dcl/one-ui/apps/sites/provisional/profile/icons";
import {
  ProfileButton,
  ProfileMoreButton,
} from "@dcl/one-ui/apps/sites/provisional/profile/ProfileActions";
import { ProfileAvatarBody } from "@dcl/one-ui/apps/sites/provisional/profile/ProfileAvatarBody";
import { ProfileHeader } from "@dcl/one-ui/apps/sites/provisional/profile/ProfileHeader";
import { ProfileLayout } from "@dcl/one-ui/apps/sites/provisional/profile/ProfileLayout";
import { ProfileMobileNav } from "@dcl/one-ui/apps/sites/provisional/profile/ProfileMobileNav";
import {
  ProfileOverview,
  type ProfileBadgeItem,
} from "@dcl/one-ui/apps/sites/provisional/profile/ProfileOverview";
import {
  ProfileTabs,
  type TabLinkProps,
} from "@dcl/one-ui/apps/sites/provisional/profile/ProfileTabs";

import type { ProfileVM } from "@/data/lib/catalyst/social/profile";

import { useMediaQuery } from "./useMediaQuery";

/** A NAME is bought in the Shop; worlds are managed in the Builder, both served by path. */
export const GET_A_NAME_HREF = "/shop/items?category=names";
export const MANAGE_WORLD_HREF = "/builder/worlds";
/** The profile site's tablet-and-below breakpoint: a phone shows the section list in place of the tab bar. */
export const MOBILE_QUERY = "(max-width: 991.95px)";

const INFO_ICONS: Record<string, ReactNode> = {
  country: <Icon name="PublicRounded" />,
  language: <Icon name="TranslateRounded" />,
  pronouns: <PronounsIcon />,
  relationship_status: <Icon name="FavoriteBorderRounded" />,
  gender: <Icon name="TransgenderRounded" />,
  profession: <Icon name="SportsEsportsOutlined" />,
  real_name: <Icon name="AlternateEmailRounded" />,
  favorite_hobby: <Icon name="FavoriteBorderRounded" />,
};

const TAB_ICONS: Record<string, IconName> = {
  overview: "PersonOutlined",
  assets: "WorkOutlineOutlined",
  creations: "BrushOutlined",
  communities: "GroupsOutlined",
  places: "LandscapeOutlined",
  photos: "PhotoOutlined",
  "referral-rewards": "CardGiftcardOutlined",
};

function TabLink({ to, ...props }: TabLinkProps) {
  return <Link to={to} {...props} />;
}

export type ProfileOverviewScreenProps = {
  profile: ProfileVM;
  own: boolean;
  tabs: { id: string; label: string; href: string }[];
  onTab: (id: string) => void;
  /** Whether the URL names a tab; without one a phone shows the section list. */
  explicitTab?: boolean;
  /** A signed-in visitor; a guest is sent to sign in before adding a friend. */
  signedIn?: boolean;
  badges?: readonly ProfileBadgeItem[];
  /** The link the owner's Invite friends / Share Profile copies. */
  shareHref?: string;
  onSignIn?: () => void;
  onLogout?: () => void;
  onEdit?: () => void;
};

/**
 * The name and avatar color every Decentraland client derives from the display name (the
 * explorer's NameColorHelper), not the profile's stored color.
 */
export function profileNameColor(profile: ProfileVM): string {
  return getAvatarBackgroundColor(getDisplayName(profile.name, profile.hasClaimedName, profile.address));
}

/** Copies `href` as an absolute link; the button says so for a moment. */
function useCopyLink(href: string | undefined): [label: boolean, copy: () => void] {
  const [copied, setCopied] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => () => void (timer.current && clearTimeout(timer.current)), []);
  const copy = () => {
    if (!href) return;
    const url = new URL(href, window.location.origin).href;
    void navigator.clipboard?.writeText(url).then(() => {
      setCopied(true);
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(() => setCopied(false), 1500);
    });
  };
  return [copied, copy];
}

/** Overview with the profile site's look: header, tab bar, avatar column, Badges, About, Links and Equipped items. */
export function ProfileOverviewScreen({
  profile,
  own,
  tabs,
  onTab,
  explicitTab = false,
  signedIn = own,
  badges = [],
  shareHref,
  onSignIn,
  onLogout,
  onEdit,
}: ProfileOverviewScreenProps) {
  const nameColor = profileNameColor(profile);
  const mobile = useMediaQuery(MOBILE_QUERY);
  const [shareCopied, copyShare] = useCopyLink(shareHref);
  const nameCta = profile.hasClaimedName ? (
    <ProfileButton href={MANAGE_WORLD_HREF} icon="Public">
      Manage World
    </ProfileButton>
  ) : (
    <ProfileButton href={GET_A_NAME_HREF} icon="Verified">
      Get a Unique Name
    </ProfileButton>
  );
  const addFriend = (size: "friend" | "drawer") =>
    signedIn ? (
      <ProfileButton icon="PersonAdd" size={size} disabled title="Friend requests are sent in world">
        Add friend
      </ProfileButton>
    ) : (
      <ProfileButton icon="PersonAdd" size={size} onClick={onSignIn}>
        Add friend
      </ProfileButton>
    );

  // No top-bar entry is highlighted on a profile ("play" maps to none), as on the profile site.
  if (mobile && !explicitTab) {
    return (
      <SitesChrome active="play" overlayNav>
        <ProfileLayout showAside={false}>
          <ProfileMobileNav
            address={profile.address}
            name={profile.name}
            nameColor={nameColor}
            avatarUrl={profile.avatarUrl || undefined}
            sections={tabs.map((tab) => ({ ...tab, icon: TAB_ICONS[tab.id] ?? "PersonOutlined" }))}
            cta={
              own ? (
                <ProfileButton variant="outlined" icon="ShareOutlined" size="drawer" onClick={copyShare}>
                  {shareCopied ? "Link copied!" : "Share Profile"}
                </ProfileButton>
              ) : (
                addFriend("drawer")
              )
            }
            onSelect={onTab}
            onLogout={own ? onLogout : undefined}
            LinkComponent={TabLink}
          />
        </ProfileLayout>
      </SitesChrome>
    );
  }

  const actions = own ? (
    <>
      {nameCta}
      <ProfileButton variant="outlined" icon="ShareOutlined" onClick={copyShare}>
        {shareCopied ? "Link copied!" : "Invite friends"}
      </ProfileButton>
    </>
  ) : (
    <>
      {addFriend("friend")}
      <ProfileMoreButton disabled={!signedIn} />
    </>
  );

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
          <ProfileTabs tabs={tabs} active="overview" onSelect={onTab} LinkComponent={TabLink} />
        }
        aside={<ProfileAvatarBody imageUrl={profile.bodyUrl || undefined} name={profile.name} />}
      >
        <ProfileOverview
          bio={profile.bio}
          emptyBio={
            own
              ? "No information yet. Jump in world and complete your bio!"
              : "This member has not added a bio yet."
          }
          info={profile.info.map((item) => ({ ...item, icon: INFO_ICONS[item.key] }))}
          links={profile.links}
          badges={badges}
          equipped={profile.equipped}
          onEdit={own ? onEdit : undefined}
        />
      </ProfileLayout>
    </SitesChrome>
  );
}
