import { useContext, useId, type MouseEvent, type ReactNode } from "react";
import { Link } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { NavCreditsBalance } from "@dcl/one-ui";
import { useChromeAuth } from "@dcl/one-ui/components/AppChrome/chrome-auth";
import SiteLayout from "@dcl/one-ui/components/AppChrome/SiteLayout";
import SitesFooter, { type FooterCopy } from "@dcl/one-ui/apps/sites/chrome/SitesFooter";
import SitesSectionNavigation, { type SitesSectionItem } from "@dcl/one-ui/apps/sites/chrome/SitesSectionNavigation";
import { ProfileNavigationContext } from "@dcl/one-ui/apps/sites/chrome/profile-navigation";
import { SITES_NAV_AREAS, type SitesNavId } from "@dcl/one-ui/apps/sites/chrome/sites-navigation";
import { creditsBalanceQuery, formatCreditsBalance } from "@shop/credits/balance-query";
import { docsUrl } from "@web-core/docs";
import { subscribeSiteNewsletter } from "@web-core/newsletter";

import { sitesShopClient } from "@/data/lib/auth/shop-client";

import { useChromeClicks } from "./chrome-clicks";
import SitesTopBar from "./SitesTopBar";
import styles from "./SitesLayout.module.css";

type SitesLayoutProps = {
  children?: ReactNode;
  active?: SitesNavId;
  signedIn?: boolean;
  account?: string;
  onSignIn?: () => void;
  overlayNav?: boolean | "mobile";
  headerSurface?: "solid" | "overlay";
  hideNav?: boolean;
  footerCopy?: FooterCopy;
  secondaryNavigation?: { title: string; active?: string; items: readonly SitesSectionItem[] };
};

const CREDITS_HREF = "/shop/credits";

function SitesCreditsChip({ account }: { account: string }) {
  const { data, isLoading, isError } = useQuery(creditsBalanceQuery(account, sitesShopClient()));
  const clicks = useChromeClicks();
  if (isLoading || isError) return null;
  const shown = formatCreditsBalance(data?.credits ?? 0);
  const onClick = (event: MouseEvent<HTMLAnchorElement>) =>
    clicks.navbarLink(CREDITS_HREF, event, { signedIn: true });
  return (
    <NavCreditsBalance
      as={Link}
      to={CREDITS_HREF}
      label={shown}
      aria-label={`${shown} credits`}
      onClick={onClick}
      onAuxClick={onClick}
    />
  );
}

export default function SitesLayout({
  children,
  active = "whatson",
  signedIn,
  account,
  onSignIn,
  overlayNav = false,
  headerSurface = "solid",
  hideNav = false,
  footerCopy,
  secondaryNavigation,
}: SitesLayoutProps) {
  const pageId = useId();
  const clicks = useChromeClicks();
  const auth = useChromeAuth();
  const profile = useContext(ProfileNavigationContext);
  const isIn = signedIn ?? auth.signedIn;
  const address = account ?? auth.account;
  const layoutClass = overlayNav === "mobile" ? styles.mobileOverlay : overlayNav ? styles.overlay : undefined;
  const section = profile
    ? { title: "Profile", items: profile.tabs, active: profile.active, onSelect: profile.onTab }
    : secondaryNavigation;

  return (
    <SiteLayout
      className={layoutClass ? `${styles.root} ${layoutClass}` : styles.root}
      overlayNavigation={headerSurface === "overlay"}
      navigation={hideNav ? undefined : (
        <SitesTopBar
          variant="sites"
          transparent={headerSurface === "overlay"}
          active={SITES_NAV_AREAS[active]}
          signedIn={isIn}
          account={address}
          onSignIn={onSignIn ?? auth.onSignIn}
          fetchNotifications={auth.fetchNotifications}
          balances={isIn && address ? <SitesCreditsChip account={address} /> : undefined}
        />
      )}
      sections={section ? (closeMenu) => <SitesSectionNavigation {...section} closeMenu={closeMenu} /> : undefined}
    >
      <div className={styles.body}>
        <a className={styles.skip} href={`#${pageId}`}>Skip to content</a>
        <div className={styles.page} id={pageId} tabIndex={-1}>{children}</div>
        <SitesFooter
          footerCopy={footerCopy}
          docsUrl={docsUrl}
          onSubscribe={subscribeSiteNewsletter}
          onSocialClick={clicks.footerSocial}
          onLinkClick={clicks.footerLink}
        />
      </div>
    </SiteLayout>
  );
}

export function SitesLayoutMaybe({ chrome = true, children, ...rest }: SitesLayoutProps & { chrome?: boolean }) {
  return chrome ? <SitesLayout {...rest}>{children}</SitesLayout> : <>{children}</>;
}
