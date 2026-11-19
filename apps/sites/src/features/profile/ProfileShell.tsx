import { useEffect, useId, type MouseEvent, type ReactNode } from "react";
import { Link } from "react-router";

import SitesChrome from "@ui/web/frames/SitesChrome";

import { Button, Tabs, TabsBar, type TabsLinkProps } from "@dcl/one-ui";

import { SHOP_BASE } from "@/core/lib/shop/redirects";
import { shortAddress } from "@/data/lib/catalyst/format/address";
import type { ProfileVM } from "@/data/lib/catalyst/social/profile";

import { ProfileHeader } from "./ProfileHeader";
import { ProfileFriendsCount, ProfileMemberActions } from "./ProfileMemberActions";
import styles from "./ProfileShell.module.css";
import { revealTab } from "./reveal-tab";

/**
 * A NAME is bought in the Shop's NAMEs page; worlds are managed under Create. Both are the
 * canonical paths, not the redirects (`/shop/items?category=names`, the legacy `/builder/worlds`).
 */
export const GET_A_NAME_HREF = `${SHOP_BASE}/names`;
export const MANAGE_WORLD_HREF = "/create/worlds";

export type ProfileShellTab = { id: string; label: string; href: string };

/** Loads the next tab's code on hover or focus, as the app's other in-app links do. */
function TabLink({ href, ...props }: TabsLinkProps) {
  return <Link to={href} prefetch="intent" {...props} />;
}

/** A modifier or middle click opens a new browser tab; this page's tab does not change. */
function isPlainClick(event: MouseEvent<HTMLAnchorElement>): boolean {
  return event.button === 0 && !event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey;
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
  const ids = useId();
  const tabId = (id: string) => `${ids}-tab-${id}`;
  // On a narrow screen the bar scrolls sideways and opens at its start, where a late tab is out of sight.
  const activeTabId = tabId(active);
  useEffect(() => {
    const reveal = () => revealTab(document.getElementById(activeTabId));
    reveal();
    // A turned phone or a resized window can put the tab out of sight again.
    window.addEventListener("resize", reveal);
    return () => window.removeEventListener("resize", reveal);
  }, [activeTabId]);
  const panelId = `${ids}-panel`;

  const actions = own ? (
    <>
      <ProfileFriendsCount address={profile.address} />
      {profile.hasClaimedName ? (
        <Button href={MANAGE_WORLD_HREF} size="md">
          Manage World
        </Button>
      ) : (
        <Button href={GET_A_NAME_HREF} size="md">
          Get a Unique Name
        </Button>
      )}
    </>
  ) : (
    // Keyed by address: another profile starts with no result of the last one's actions.
    <ProfileMemberActions key={profile.address} address={profile.address} name={profile.name} />
  );

  // No top-bar entry is highlighted on a profile ("play" maps to none), as on the profile site.
  return (
    <SitesChrome active="play" overlayNav>
      <div className={styles.root}>
        <div className={styles.content}>
          <ProfileHeader
            address={profile.address}
            addressLabel={shortAddress(profile.address)}
            name={profile.name}
            nameColor={profile.nameColor}
            hasClaimedName={profile.hasClaimedName}
            avatarUrl={profile.avatarUrl || undefined}
            actions={actions}
          />
          <TabsBar label="Profile sections" inset="content" scrollable>
            {tabs.map((tab) => (
              <Tabs
                key={tab.id}
                id={tabId(tab.id)}
                label={tab.label}
                href={tab.href}
                linkComponent={TabLink}
                selected={tab.id === active}
                aria-controls={tab.id === active ? panelId : undefined}
                onClick={(event) => {
                  if (isPlainClick(event)) onTab(tab.id);
                }}
              />
            ))}
          </TabsBar>
          <div
            role="tabpanel"
            id={panelId}
            aria-labelledby={tabId(active)}
            className={aside ? `${styles.body} ${styles.withAside}` : styles.body}
          >
            {aside ? <aside className={styles.aside}>{aside}</aside> : null}
            <div className={styles.main}>{children}</div>
          </div>
        </div>
      </div>
    </SitesChrome>
  );
}
