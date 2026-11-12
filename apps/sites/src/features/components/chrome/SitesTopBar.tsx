import {
  sitesNavigationLinks,
  sitesNavigationOrder,
} from "@dcl/one-ui/apps/sites/chrome/navigation";
import { type CSSProperties, useEffect, useMemo, useState } from "react";
import { useLocation } from "react-router";
import { NavBar, type NavBarLinkProps, type NavBarSection } from "@dcl/one-ui";
import type { DclTopBarProps } from "@ui/web/frames/DclTopBar";
import { useChromeAuth } from "@ui/web/frames/chrome-auth";
import ChromeLink from "@ui/web/frames/ChromeLink";
import { useSiteLayout } from "@ui/web/frames/SiteLayout";
import { docsUrl } from "@ui/data/docs";
import {
  notificationBody,
  notificationLink,
  notificationTitle,
  relativeTime,
} from "@ui/data/catalyst/notificationsView";
import SitesNotificationsDialog from "@dcl/one-ui/apps/sites/chrome/SitesNotificationsDialog";
import logo from "@ui/atoms/assets/decentraland-logo.svg?url";
import wordmark from "@ui/atoms/assets/decentraland-wordmark.svg?url";
import { useChromeNotifications } from "./useChromeNotifications";
import styles from "./SitesTopBar.module.css";

/** decentraland.org's marks (sites: components/LandingNavbar/icons.tsx DclLogo, HamburgerIcon, BellIcon) in the NavBar's slots. */
const SITE_ICONS = {
  logo: <img src={logo} width={40} height={40} alt="" />,
  menu: (
    <svg
      className={styles.menuIcon}
      width="24"
      height="24"
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
    >
      <path d="M3 6H21" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
      <path d="M3 12H21" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
      <path d="M3 18H21" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
    </svg>
  ),
  notifications: (
    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <path
        d="M12 21.75C13.1 21.75 14 20.85 14 19.75H10C10 20.85 10.89 21.75 12 21.75ZM18 15.75V10.75C18 7.68 16.36 5.11 13.5 4.43V3.75C13.5 2.92 12.83 2.25 12 2.25C11.17 2.25 10.5 2.92 10.5 3.75V4.43C7.63 5.11 6 7.67 6 10.75V15.75L4 17.75V18.75H20V17.75L18 15.75Z"
        fill="currentColor"
      />
    </svg>
  ),
};

export function sitesNavSections(
  active: DclTopBarProps["active"],
  pathname: string,
): NavBarSection[] {
  const playActive =
    active === "whatson" || active === "events" || active === "explore";
  const links = sitesNavigationLinks(docsUrl());
  return sitesNavigationOrder.map((id) => {
    const link = links[id];
    const selected = id === "whatson" ? playActive : active === id;
    if (!("menu" in link)) {
      return { label: link.label, href: link.href, active: selected };
    }
    return {
      label: link.label,
      active: selected,
      items: link.menu.map((item, index) => ({
        ...item,
        active:
          id === "whatson"
            ? item.href === "/play/"
              ? pathname.startsWith("/play")
              : playActive
            : selected && pathname.startsWith(index === 0 ? "/docs" : "/blog"),
      })),
    };
  });
}

/** The Sites host adapts real chrome state; the shared component owns navigation UI. */
export default function SitesTopBar({
  active,
  signedIn,
  account,
  onSignIn,
  onSignOut,
  signInHref = "/shop/my-items",
  fetchNotifications,
  mobileNavigation,
  accountControl,
  balances,
  onNavigate,
}: DclTopBarProps) {
  const NavigationLink = useMemo(
    () =>
      function NavigationLink(props: NavBarLinkProps) {
        return <ChromeLink {...props} onNavigate={onNavigate} />;
      },
    [onNavigate],
  );
  const auth = useChromeAuth();
  const layout = useSiteLayout();
  const { pathname } = useLocation();
  const isIn = signedIn ?? auth.signedIn;
  const address = account ?? auth.account;
  const signIn = onSignIn ?? auth.onSignIn;
  const signOut = onSignOut ?? auth.onSignOut;
  const switchAccount =
    auth.onSwitchAccount ??
    (signOut && signIn
      ? () => {
          signOut();
          signIn();
        }
      : undefined);
  const [notificationAccount, setNotificationAccount] = useState<string | null>(
    null,
  );
  const identity = isIn && address ? address.toLowerCase() : null;
  const notificationsOpen =
    identity !== null && notificationAccount === identity;
  const load = fetchNotifications ?? auth.fetchNotifications;
  useEffect(() => {
    setNotificationAccount(null);
  }, [identity]);
  const notifications = useChromeNotifications(
    identity,
    notificationsOpen,
    load,
  );
  const rows = notifications.data;
  const actions = useMemo(
    () => [
      {
        label: "My profile",
        href: `/profile/${encodeURIComponent(address.toLowerCase())}`,
      },
      { label: "My assets", href: "/shop/my-items" },
      ...(switchAccount
        ? [{ label: "Switch account", onSelect: switchAccount }]
        : []),
      ...(signOut ? [{ label: "Log out", onSelect: signOut }] : []),
    ],
    [address, switchAccount, signOut],
  );
  const sections = mobileNavigation ?? layout?.sections;
  const profileMatches =
    identity !== null && auth.account.toLowerCase() === identity;
  // The landing page's nav, as decentraland.org's Layout derives it from the path: signed out it is
  // bare (logo, wordmark, Sign in); signed out or in, the desktop bar floats transparent over the hero.
  const landing = pathname === "/";
  const bare = landing && !isIn;
  const avatarColor = profileMatches ? auth.avatarColor : undefined;
  return (
    <>
      <header
        aria-label="Decentraland"
        className={styles.root}
        data-landing={landing || undefined}
        data-bare={bare || undefined}
        style={
          avatarColor
            ? ({
                "--dcl-navbar-avatar-background": avatarColor,
              } as CSSProperties)
            : undefined
        }
      >
        <NavBar
          sections={bare ? [] : sitesNavSections(active, pathname)}
          brand={
            bare ? (
              <img
                className={styles.wordmark}
                src={wordmark}
                width={160}
                height={20}
                alt="Decentraland"
              />
            ) : undefined
          }
          icons={SITE_ICONS}
          linkComponent={NavigationLink}
          {...(identity
            ? {
                account: {
                  address,
                  ...(profileMatches && auth.name ? { name: auth.name } : {}),
                  ...(profileMatches && auth.avatarUrl
                    ? { avatarUrl: auth.avatarUrl }
                    : {}),
                },
              }
            : { signIn: signIn ? { onSelect: signIn } : { href: signInHref } })}
          balanceAction={isIn ? balances : undefined}
          accountControl={accountControl}
          accountActions={actions}
          mobileSecondaryNavigation={
            sections ? ({ closeDrawer }) => sections(closeDrawer) : undefined
          }
          notifications={
            identity
              ? {
                  ...(rows
                    ? { count: rows.filter((row) => !row.read).length }
                    : {}),
                  onSelect: () => setNotificationAccount(identity),
                }
              : undefined
          }
        />
      </header>
      {notificationsOpen ? (
        <SitesNotificationsDialog
          onClose={() => setNotificationAccount(null)}
          state={
            notifications.isPending
              ? "loading"
              : notifications.isError
                ? "error"
                : "ready"
          }
          rows={(rows ?? []).slice(0, 10).map((row) => ({
            id: row.id,
            title: notificationTitle(row),
            description: notificationBody(row),
            href: notificationLink(row) ?? undefined,
            time: relativeTime(row.timestamp, Date.now()),
            unread: !row.read,
          }))}
          Link={NavigationLink}
        />
      ) : null}
    </>
  );
}
