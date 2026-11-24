import type { ReactNode } from "react";
import { ChromeBody, type ChromeNavigate, ChromeLink, SiteLayout } from "@dcl/one-ui";
import DappFooter from "./DappFooter";
import DclTopBar from "../web/frames/DclTopBar";
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
