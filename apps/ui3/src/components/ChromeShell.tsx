import type { ReactNode } from "react";
import ChromeBody from "@dcl/one-ui/components/AppChrome/ChromeBody";
import DappFooter from "./DappFooter";
import type { ChromeNavigate } from "@dcl/one-ui/components/AppChrome/chrome-nav";
import ChromeLink from "@dcl/one-ui/components/AppChrome/ChromeLink";
import DclTopBar from "../web/frames/DclTopBar";
import SiteLayout from "@dcl/one-ui/components/AppChrome/SiteLayout";
import SiteSectionNavigation from "../web/frames/SiteSectionNavigation";
import "./chromeshell.css";

type ChromeTab<Id extends string = string> = {
  id: Id;
  label: ReactNode;
  href?: string;
  icon?: ReactNode;
};

type ChromeShellProps<Id extends string = string> = {
  className?: string;
  ariaLabel?: string;
  topbar?: ReactNode;
  subnav?: boolean;
  brand?: ReactNode;
  brandHref?: string;
  sectionTitle?: ReactNode;
  tabs?: readonly ChromeTab<Id>[];
  active?: NoInfer<Id>;
  onTab?: (id: NoInfer<Id>) => void;
  onNavigate?: ChromeNavigate;
  tabsLabel?: string;
  right?: ReactNode;
  children?: ReactNode;
  footer?: ReactNode;
  extraNavigation?: ReactNode;
};

export default function ChromeShell<Id extends string = string>({
  className = "", ariaLabel, topbar, subnav = true, brand = null, brandHref, sectionTitle,
  tabs = [], active, onTab, onNavigate, tabsLabel, right = null, children, footer, extraNavigation,
}: ChromeShellProps<Id>) {
  return (
    <SiteLayout className="ui2" navigation={topbar ?? <DclTopBar active="" />}
      sections={(subnav && tabs.length) || extraNavigation ? closeMenu => <>
        {subnav && tabs.length ? <SiteSectionNavigation
          title={sectionTitle === undefined ? (brand ? (brandHref ? <ChromeLink href={brandHref} onNavigate={onNavigate}>{brand}</ChromeLink> : brand) : ariaLabel ?? "Sections") : sectionTitle}
          label={tabsLabel} items={tabs} active={active} onSelect={onTab} onNavigate={onNavigate} closeMenu={closeMenu}
        /> : null}
        <div onClick={event => { if ((event.target as Element).closest("button, a")) closeMenu(); }}>{extraNavigation}</div>
      </> : undefined}
    >
      <ChromeBody className={className} ariaLabel={ariaLabel} tools={right} footer={footer === false ? null : footer ?? <DappFooter />}>
        {children}
      </ChromeBody>
    </SiteLayout>
  );
}
