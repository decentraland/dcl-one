import { useContext, useId, type ReactNode } from "react";
import DclTopBar from "./DclTopBar";
import SitesCreditsChip from "./SitesCreditsChip";
import { useChromeAuth } from "@dcl/one-ui/components/AppChrome/chrome-auth";
import "../../atoms/maskicon.css";
import { docsUrl } from "@web-core/docs";
import "./siteschrome.css";
import { SITES_NAV_AREAS, type SitesNavId } from "@dcl/one-ui/apps/sites/chrome/sites-navigation";
export type { SitesNavId } from "@dcl/one-ui/apps/sites/chrome/sites-navigation";
import SitesFooter, { type FooterCopy } from "@dcl/one-ui/apps/sites/chrome/SitesFooter";
import { subscribeSiteNewsletter } from "@web-core/newsletter";
import SiteLayout from "@dcl/one-ui/components/AppChrome/SiteLayout";
import SiteSectionNavigation from "./SiteSectionNavigation";
import { ProfileNavigationContext } from "@dcl/one-ui/apps/sites/chrome/profile-navigation";

type SitesChromeProps = {
  children?: ReactNode;
  active?: SitesNavId;
  signedIn?: boolean;
  mana?: string;
  account?: string;
  onSignIn?: () => void;
  overlayNav?: boolean | "mobile";
  headerSurface?: "solid" | "overlay";
  hideNav?: boolean;
  footerCopy?: FooterCopy;
  secondaryNavigation?: { title: string; active?: string; items: readonly { id: string; label: string; href: string }[] };
};

export default function SitesChrome({
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
}: SitesChromeProps) {
  const pageId = useId();
  const profileNav = useContext(ProfileNavigationContext);
  const auth = useChromeAuth();
  const isIn = signedIn ?? auth.signedIn;
  const acct = account ?? auth.account;
  const signIn = onSignIn ?? auth.onSignIn;

  const overlayClass =
    overlayNav === "mobile" ? " site-layout--overlay-mobile"
    : overlayNav ? " site-layout--overlay"
    : "";

  return (
    <SiteLayout overlayNavigation={headerSurface === "overlay"} navigation={!hideNav ? <DclTopBar
      variant="sites" transparent={headerSurface === "overlay"} active={SITES_NAV_AREAS[active]} signedIn={isIn} account={acct}
      onSignIn={signIn} fetchNotifications={auth.fetchNotifications}
      balances={isIn && acct ? <SitesCreditsChip account={acct} /> : undefined}
    /> : null} className={"ui2 site-layout--sites" + overlayClass} sections={profileNav ? closeMenu => <SiteSectionNavigation
      title="Profile" label="Profile sections" items={profileNav.tabs} active={profileNav.active}
      onSelect={profileNav.onTab} closeMenu={closeMenu}
    /> : secondaryNavigation ? closeMenu => <SiteSectionNavigation
      title={secondaryNavigation.title} items={secondaryNavigation.items} active={secondaryNavigation.active} closeMenu={closeMenu}
    /> : undefined}>
    <div className="st ui2" data-label="Decentraland">
      <a className="st__skip" href={"#" + pageId}>Skip to content</a>

      <div className="st__page" id={pageId} tabIndex={-1}>{children}</div>

      <SitesFooter footerCopy={footerCopy} docsUrl={docsUrl} onSubscribe={subscribeSiteNewsletter} />
    </div>
    </SiteLayout>
  );
}

export function SitesChromeMaybe({
  chrome = true,
  children,
  ...rest
}: SitesChromeProps & { chrome?: boolean }) {
  if (!chrome) return <>{children}</>;
  return <SitesChrome {...rest}>{children}</SitesChrome>;
}
