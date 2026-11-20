import {
  type CSSProperties,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import { useLocation } from "react-router";
import {
  decentralandPublicNavSections,
  NavBar,
  GeneralMarketplace,
  GeneralPlayers,
  MenusLeave,
  GeneralWallet,
  GeneralSecondaryBarSettingsOutline,
  GeneralSecondaryBarWearables,
  type NavBarLinkProps,
} from "@dcl/one-ui";
import { chainLabel } from "@shop/model/chain";
import { browserProvider, web3Usable } from "@/data/lib/auth/shop-wallet";
import { ethereumChainId, settlementChainId } from "@ui/shop/checkout/chain/config";
import { manaWeiToNumber } from "@ui/shop/checkout/lib/mana-format";
import { useManaBalance } from "@ui/shop/credits/hooks";
import { useWalletAccount, useWalletChain } from "@ui/shop/wallet/context";
import type { Eip1193Provider } from "@ui/data/auth/wallet";
import type { DclTopBarProps } from "@ui/web/frames/DclTopBar";
import { useChromeAuth } from "@ui/web/frames/chrome-auth";
import ChromeLink from "@ui/web/frames/ChromeLink";
import { useSiteLayout } from "@ui/web/frames/SiteLayout";
import {
  notificationBody,
  notificationLink,
  notificationTitle,
  relativeTime,
  unreadCount,
} from "@ui/data/catalyst/notificationsView";
import SitesNotificationsDialog from "@dcl/one-ui/apps/sites/chrome/SitesNotificationsDialog";
import logo from "@ui/atoms/assets/decentraland-logo.svg?url";
import wordmark from "@dcl/one-ui/apps/sites/assets/dcl_name.svg?url";
import { SitesLocaleContext } from "@ui/web/frames/sites-locale";
import { useMarketplaceHere } from "@/data/lib/shop/marketplace-access";
import { navProfileCopy } from "./nav-profile-copy";
import { useChromeNotifications } from "./useChromeNotifications";
import styles from "./SitesTopBar.module.css";

/** decentraland.org's marks in the NavBar's slots. */
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
      <path
        d="M3 6H21"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
      />
      <path
        d="M3 12H21"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
      />
      <path
        d="M3 18H21"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
      />
    </svg>
  ),
  notifications: (
    <svg
      width="24"
      height="24"
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
    >
      <path
        d="M12 21.75C13.1 21.75 14 20.85 14 19.75H10C10 20.85 10.89 21.75 12 21.75ZM18 15.75V10.75C18 7.68 16.36 5.11 13.5 4.43V3.75C13.5 2.92 12.83 2.25 12 2.25C11.17 2.25 10.5 2.92 10.5 3.75V4.43C7.63 5.11 6 7.67 6 10.75V15.75L4 17.75V18.75H20V17.75L18 15.75Z"
        fill="currentColor"
      />
    </svg>
  ),
};

function ProfileNetwork({ provider, label }: { provider: Eip1193Provider; label: string }) {
  const { chainId, pendingChainId, switchTo } = useWalletChain(provider);
  if (chainId === null) return null;
  const ethereum = ethereumChainId();
  const polygon = settlementChainId();
  return (
    <select
      aria-label={label}
      className={styles.profileNetwork}
      value={pendingChainId ?? chainId}
      disabled={pendingChainId !== null}
      onChange={(event) => void switchTo(Number(event.currentTarget.value))}
    >
      {chainId !== ethereum && chainId !== polygon ? (
        <option value={chainId} disabled>{chainLabel(chainId)}</option>
      ) : null}
      <option value={ethereum}>{chainLabel(ethereum)}</option>
      <option value={polygon}>{chainLabel(polygon)}</option>
    </select>
  );
}

/** The Sites host adapts real chrome state; the shared component owns navigation UI. */
export default function SitesTopBar({
  active,
  signedIn,
  account,
  transparent,
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
  const { pathname, search } = useLocation();
  const landing = pathname === "/";
  const locale = useContext(SitesLocaleContext)?.locale ?? "en";
  const { labels, actions: actionLabels, walletNetwork } = navProfileCopy[locale];
  const [profileOpen, setProfileOpen] = useState(false);
  const isIn = signedIn ?? auth.signedIn;
  const address = account ?? auth.account;
  const signIn = onSignIn ?? auth.onSignIn;
  const signOut = onSignOut ?? auth.onSignOut;
  const switchAccount = auth.onSwitchAccount;
  const [notificationOwner, setNotificationOwner] = useState<{
    account: string;
    pathname: string;
  } | null>(null);
  const identity = isIn && address ? address.toLowerCase() : null;
  const marketplaceAvailable = useMarketplaceHere(identity);
  const profileWalletAvailable = identity !== null && web3Usable({ signer: identity });
  const provider = profileWalletAvailable ? browserProvider() : null;
  const walletAccount = useWalletAccount(provider);
  const matchingWallet = provider !== null && identity !== null && walletAccount === identity;
  const marketplaceEligible = marketplaceAvailable && matchingWallet;
  const notificationsOpen =
    identity !== null &&
    notificationOwner?.account === identity &&
    notificationOwner.pathname === pathname;
  const load = fetchNotifications ?? auth.fetchNotifications;
  useEffect(() => {
    setNotificationOwner(null);
  }, [identity, pathname]);

  const notifications = useChromeNotifications(
    identity,
    notificationsOpen,
    load,
    landing && auth.notificationsReady !== false,
  );
  const rows = notifications.data;
  const profileManaEligible = profileWalletAvailable && matchingWallet;
  const manaAccount = profileOpen && profileManaEligible ? identity : null;
  const { data: mana, isPending: manaPending } = useManaBalance(manaAccount, ethereumChainId());
  const { data: polygonMana, isPending: polygonManaPending } = useManaBalance(manaAccount, settlementChainId());
  const menuBalances = manaAccount
    ? {
        mana: mana === undefined ? undefined : manaWeiToNumber(mana),
        polygonMana: polygonMana === undefined ? undefined : manaWeiToNumber(polygonMana),
      }
    : undefined;
  const actions = useMemo(
    () => [
      {
        label: actionLabels.viewProfile,
        icon: <GeneralPlayers size={20} aria-hidden="true" />,
        href: `/profile/${encodeURIComponent(address.toLowerCase())}`,
      },
      {
        label: actionLabels.myAssets,
        href: "/shop/my-items",
        icon: <GeneralSecondaryBarWearables size={20} aria-hidden="true" />,
      },
      {
        label: actionLabels.accountSettings,
        href: "/account",
        icon: <GeneralSecondaryBarSettingsOutline size={20} aria-hidden="true" />,
      },
      {
        label: actionLabels.shopAuthorizations,
        href: "/shop/settings",
        icon: <GeneralMarketplace size={20} aria-hidden="true" />,
      },
      ...(signOut
        ? [
            {
              label: actionLabels.logout,
              onSelect: signOut,
              icon: <MenusLeave size={20} aria-hidden="true" />,
              separated: true,
            },
          ]
        : switchAccount
          ? [
              {
              label: actionLabels.switchAccount,
                onSelect: switchAccount,
                icon: <GeneralWallet size={20} aria-hidden="true" />,
                separated: true,
              },
            ]
          : []),
    ],
    [address, actionLabels, switchAccount, signOut],
  );
  const panel = notificationsOpen ? (
    <SitesNotificationsDialog
      presentation={landing ? "popover" : "modal"}
      onClose={() => setNotificationOwner(null)}
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
  ) : null;
  const sections = mobileNavigation ?? layout?.sections;
  const profileMatches =
    identity !== null && auth.account.toLowerCase() === identity;
  // The landing page's nav, as decentraland.org's Layout derives it from the path: signed out it is
  // bare (logo, wordmark, Sign in); signed out or in, the desktop bar floats transparent over the hero.
  const bare = landing && !isIn;
  const { desktop, mobile } = bare
    ? { desktop: [], mobile: [] }
    : decentralandPublicNavSections(active, pathname, { search, marketplaceEligible });
  const avatarColor = profileMatches ? auth.avatarColor : undefined;
  return (
    <>
      <header
        aria-label="Decentraland"
        className={styles.root}
        data-landing={landing || undefined}
        data-transparent={(transparent && !landing) || undefined}
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
          sections={desktop}
          mobileSections={mobile}
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
                  ...(profileMatches && auth.bodyUrl ? { bodyUrl: auth.bodyUrl } : {}),
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
          accountMenuAppearance="profile"
          accountMenuBalances={menuBalances}
          accountMenuBalancesPending={profileManaEligible && (manaPending || polygonManaPending)}
          accountMenuNetwork={manaAccount && provider ? (
            <ProfileNetwork provider={provider} label={walletNetwork} />
          ) : undefined}
          onAccountMenuOpenChange={setProfileOpen}
          labels={labels}
          locale={locale}
          mobileSecondaryNavigation={
            sections ? ({ closeDrawer }) => sections(closeDrawer) : undefined
          }
          notifications={
            identity
              ? {
                  count: unreadCount(rows),
                  ...(landing ? { expanded: notificationsOpen } : {}),
                  panel: landing ? panel : null,
                  onSelect: () => {
                    if (notificationsOpen) {
                      setNotificationOwner(null);
                      return;
                    }
                    setNotificationOwner({ account: identity, pathname });
                    if (
                      landing &&
                      notifications.isStale &&
                      !notifications.isFetching
                    ) {
                      void notifications.refetch();
                    }
                  },
                }
              : undefined
          }
        />
      </header>
      {landing ? null : panel}
    </>
  );
}
