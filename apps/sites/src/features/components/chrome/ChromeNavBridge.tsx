import { useCallback, useMemo } from "react";
import type { ComponentPropsWithRef, ReactNode } from "react";
import { Link, useNavigate } from "react-router";

import { ChromeNavContext, isSameDocumentHref, type ChromeNavigate } from "@ui/web/frames/chrome-nav";

import { legacyShopTarget } from "@/core/lib/shop/redirects";

const SERVED_OUTSIDE_THE_APP = ["/docs", "/play", "/social"] as const;

type ChromeHrefTarget = { kind: "app"; to: string } | { kind: "document" };

const UPSTREAM_SITE = /^(?:www\.)?decentraland\.(?:org|zone|today)$/;
const UPSTREAM_MARKETPLACE = /^marketplace\.decentraland\.(?:org|zone|today)$/;

/**
 * This site's Shop page for a link to the upstream Shop or Marketplace (decentraland.org/shop/...,
 * decentraland.org/marketplace/..., marketplace.decentraland.org/...), as notifications and CMS
 * content carry them: the same item, token, list or page here. Null for any other link.
 */
export function upstreamShopPath(href: string): string | null {
  let url: URL;
  try {
    url = new URL(href);
  } catch {
    return null;
  }
  if (url.protocol !== "https:" && url.protocol !== "http:") return null;
  if (UPSTREAM_MARKETPLACE.test(url.hostname)) return legacyShopTarget(`/marketplace${url.pathname}`, url.search, url.hash);
  if (!UPSTREAM_SITE.test(url.hostname)) return null;
  if (/^\/marketplace(?=\/|$)/i.test(url.pathname)) return legacyShopTarget(url.pathname, url.search, url.hash);
  if (/^\/shop(?=\/|$)/i.test(url.pathname)) return `${url.pathname}${url.search}${url.hash}`;
  return null;
}

export function chromeHrefTarget(href: string, origin: string): ChromeHrefTarget {
  let url: URL;
  try {
    url = new URL(href, origin);
  } catch {
    return { kind: "document" };
  }
  const shop = url.origin === origin ? null : upstreamShopPath(url.href);
  if (shop) return { kind: "app", to: shop };
  if (url.origin !== origin) return { kind: "document" };
  const outside = SERVED_OUTSIDE_THE_APP.some(
    (prefix) => url.pathname === prefix || url.pathname.startsWith(`${prefix}/`),
  );
  if (outside) return { kind: "document" };
  return { kind: "app", to: `${url.pathname}${url.search}${url.hash}` };
}

function SiteLink({ href = "", download, ...props }: ComponentPropsWithRef<"a">) {
  const shop = download == null ? upstreamShopPath(href) : null;
  if (shop) return <Link to={shop} prefetch="intent" {...props} />;
  if (download != null || !isSameDocumentHref(href) ||
    chromeHrefTarget(href, "https://sites.invalid").kind === "document") {
    return <a href={href} download={download} {...props} />;
  }
  return <Link to={href} prefetch="intent" {...props} />;
}

export default function ChromeNavBridge({ children }: { children: ReactNode }) {
  const navigate = useNavigate();
  const go = useCallback<ChromeNavigate>(
    (href) => {
      if (typeof window === "undefined") return false;
      const target = chromeHrefTarget(href, window.location.origin);
      if (target.kind !== "app") return false;
      void navigate(target.to);
      return true;
    },
    [navigate],
  );
  const value = useMemo(() => ({ navigate: go, Link: SiteLink }), [go]);
  return <ChromeNavContext.Provider value={value}>{children}</ChromeNavContext.Provider>;
}
