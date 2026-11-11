import { createContext, useContext } from "react";
import type {
  ComponentPropsWithRef,
  ComponentType,
  MouseEvent,
  MouseEventHandler,
  ReactNode,
} from "react";

export type ChromeNavigate = (href: string) => boolean | void | Promise<void>;

type ChromeNav = {
  navigate?: ChromeNavigate;
  Link?: ComponentType<ComponentPropsWithRef<"a">>;
  renderSitesNavBar?: (props: SitesNavBarProps) => ReactNode;
};

export type SitesNavBarProps = {
  viewport: "mobile" | "desktop" | "largeDesktop";
  signedIn: boolean;
  logoHref: string;
  labelHref: string;
  label2Href: string;
  label3Href: string;
  label4Href: string;
  onMenu: () => void;
  onNotifications: () => void;
  onProfile: () => void;
  onSignIn: () => void;
  onClick: MouseEventHandler<HTMLElement>;
};

export const ChromeNavContext = createContext<ChromeNav>({
  navigate: undefined,
});

export function useChromeNav(): ChromeNav {
  return useContext(ChromeNavContext);
}

export function isSameDocumentHref(href: string): boolean {
  return href.startsWith("/") && !href.startsWith("//");
}

export function chromeLinkClick(
  e: MouseEvent<HTMLAnchorElement>,
  href: string,
  navigate: ChromeNavigate | undefined,
): void {
  if (!navigate || e.defaultPrevented) return;
  if (e.currentTarget.hasAttribute("download")) return;
  if (e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey)
    return;
  const target = e.currentTarget.getAttribute("target");
  if (target && target !== "_self") return;
  if (!isSameDocumentHref(href)) return;
  if (navigate(href) !== false) e.preventDefault();
}

export function chromeLinkProps(
  href: string,
  navigate: ChromeNavigate | undefined,
): {
  href: string;
  onClick: (e: MouseEvent<HTMLAnchorElement>) => void;
  "data-discover"?: "true";
} {
  return {
    href,
    onClick: (e) => chromeLinkClick(e, href, navigate),
    "data-discover": navigate && isSameDocumentHref(href) ? "true" : undefined,
  };
}
