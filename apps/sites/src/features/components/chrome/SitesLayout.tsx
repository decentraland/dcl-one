import { useContext, useId, type ReactNode } from "react";
import { Link } from "react-router";
import { useQuery } from "@tanstack/react-query";
import {
  SitesFooter,
  type FooterCopy,
  SitesSectionNavigation,
  type SitesSectionItem,
  ProfileNavigationContext,
  SITES_NAV_AREAS,
  type SitesNavId,
  useChromeAuth,
  SiteLayout,
  NavCreditsBalance,
} from "@dcl/one-ui";
import { creditsBalanceQuery, formatCreditsBalance } from "@shop/credits/balance-query";
import { docsUrl } from "@web-core/docs";
import { subscribeSiteNewsletter } from "@web-core/newsletter";

import { sitesShopClient } from "@/data/lib/auth/shop-client";

import SitesTopBar from "./SitesTopBar";
import styles from "./SitesLayout.module.css";

/** One group of the sidebar (the menu on a phone): a titled list of links or buttons. */
export type SecondaryNavigation = {
  title: string;
  active?: string | null;
  items: readonly SitesSectionItem[];
  onSelect?: (id: string) => void;
};

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
  secondaryNavigation?: SecondaryNavigation | readonly SecondaryNavigation[];
};

function SitesCreditsChip({ account }: { account: string }) {
  const { data, isLoading, isError } = useQuery(creditsBalanceQuery(account, sitesShopClient()));
  if (isLoading || isError) return null;
  const shown = formatCreditsBalance(data?.credits ?? 0);
  return <NavCreditsBalance as={Link} to="/shop/credits" label={shown} aria-label={`${shown} credits`} />;
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
  const auth = useChromeAuth();
  const profile = useContext(ProfileNavigationContext);
  const isIn = signedIn ?? auth.signedIn;
  const address = account ?? auth.account;
  const layoutClass = overlayNav === "mobile" ? styles.mobileOverlay : overlayNav ? styles.overlay : undefined;
  const sections: readonly SecondaryNavigation[] = profile
    ? [{ title: "Profile", items: profile.tabs, active: profile.active, onSelect: profile.onTab }]
    : secondaryNavigation
      ? ([] as SecondaryNavigation[]).concat(secondaryNavigation)
      : [];

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
      sections={
        sections.length
          ? (closeMenu) =>
              sections.map((section, index) => (
                <SitesSectionNavigation
                  key={index}
                  {...section}
                  active={section.active ?? undefined}
                  closeMenu={closeMenu}
                />
              ))
          : undefined
      }
    >
      <div className={styles.body}>
        <a className={styles.skip} href={`#${pageId}`}>Skip to content</a>
        <div className={styles.page} id={pageId} tabIndex={-1}>{children}</div>
        <SitesFooter footerCopy={footerCopy} docsUrl={docsUrl} onSubscribe={subscribeSiteNewsletter} />
      </div>
    </SiteLayout>
  );
}

export function SitesLayoutMaybe({ chrome = true, children, ...rest }: SitesLayoutProps & { chrome?: boolean }) {
  return chrome ? <SitesLayout {...rest}>{children}</SitesLayout> : <>{children}</>;
}
