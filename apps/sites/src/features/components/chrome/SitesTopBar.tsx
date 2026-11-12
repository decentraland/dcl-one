import { useEffect, useMemo, useState } from "react";
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
import { useChromeNotifications } from "./useChromeNotifications";

export function sitesNavSections(
  active: DclTopBarProps["active"],
  pathname: string,
): NavBarSection[] {
  const playActive =
    active === "whatson" || active === "events" || active === "explore";
  return [
    {
      label: "Play",
      active: playActive,
      items: [
        {
          label: "Jump in",
          href: "/play/",
          active: pathname.startsWith("/play"),
        },
        { label: "What's on", href: "/whats-on", active: playActive },
      ],
    },
    { label: "Shop", href: "/shop", active: active === "shop" },
    { label: "Create", href: "/create", active: active === "create" },
    { label: "Social", href: "/social/", active: active === "social" },
    {
      label: "Learn",
      active: active === "learn",
      items: [
        {
          label: "Docs",
          href: docsUrl(),
          active: active === "learn" && pathname.startsWith("/docs"),
        },
        {
          label: "Blog",
          href: "/blog",
          active: active === "learn" && pathname.startsWith("/blog"),
        },
      ],
    },
  ];
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
  transparent = false,
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
  return (
    <>
      <header aria-label="Decentraland">
        <NavBar
          sections={sitesNavSections(active, pathname)}
          surface={transparent ? "overlay" : "solid"}
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
