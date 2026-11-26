import { useEffect, useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  isRouteErrorResponse,
  Links,
  Meta,
  Outlet,
  Scripts,
  ScrollRestoration,
  useRouteLoaderData,
  useMatches,
} from "react-router";

// The shared tokens, not foundations.css: the Inter faces are the app's own (@ui/atoms/inter.css, through primitives.css).
import "@dcl/one-ui/foundations/colors/colors.css";
import "@dcl/one-ui/foundations/spacing/spacing.css";
import "@dcl/one-ui/foundations/layout/layout.css";
import "@dcl/one-ui/apps/sites/profile-tokens.css";
import "@ui/atoms/primitives.css";
import {
  dclLogoUrl as logo,
  StatusPage,
  SitesLocaleContext,
  sitesLanguage,
  type SitesLocale,
  type SitesLocaleHandle,
  ThumbnailPublicBase,
} from "@dcl/one-ui";

import { QUERY_CLIENT_CONFIG } from "@/core/lib/query/defaults";
import ChromeAuthBridge from "@/features/components/chrome/ChromeAuthBridge";
import ChromeNavBridge from "@/features/components/chrome/ChromeNavBridge";
import RoutePending, { NAV_PENDING_SCRIPT } from "@/features/components/chrome/RoutePending";
import EmbedBridge from "@/features/components/chrome/EmbedBridge";
import { reloadingForAccount } from "@/features/components/chrome/explorer-host";
import { sessionChangePending, useSessionChangeRevalidation } from "@/data/lib/auth/session-change";
import NotFoundPage from "@/features/not-found/NotFoundPage";
import { EMBED_BOOT_SCRIPT } from "@/core/lib/embed";
import { isShopPath } from "@ui/shop/native-contract";
import type { ShouldRevalidateFunctionArgs } from "react-router";
import { isPublicResponsePath, pageSid, parseCookies, publicResponseContext, readVerifiedWallet, sessionMiddleware } from "@/core/lib/telemetry/session";
import { useNotFoundPageView, useRoutePageView } from "@/core/lib/telemetry/page-view";
import { sitesSegmentPublicConfig } from "@/core/lib/telemetry/public-config.server";
import { readWallet } from "@/data/lib/auth/wallet-cookie";
import { browserCatalystBase } from "@/data/lib/catalyst/bases";
import { isCommitteeMember } from "@/data/lib/catalyst/creator-hub/committee-membership.server";
import { serverShopApiMode } from "@/data/lib/shop/mode";
import { homeMeta, notFoundMeta } from "./lib/social-meta";
import type { Route } from "./+types/root";

/** Reads or mints the session id once per request, before every loader, and sets its cookie once. */
export const middleware = [sessionMiddleware];

export async function loader({ request, context }: Route.LoaderArgs) {
  const thirdwebClientId =
    (typeof process !== "undefined" && process.env.THIRDWEB_CLIENT_ID) || "";
  const walletConnectProjectId = process.env.WALLETCONNECT_PROJECT_ID || "";
  const magicApiKey = process.env.MAGIC_API_KEY || "";
  // The public Segment write key and the optional first-party proxy host
  // (core/lib/telemetry/track.ts); unset, the client's track() is a no-op.
  const { segmentWriteKey, segmentApiHost } = sitesSegmentPublicConfig();
  // The Shop's own public write key (no fallback to Sites'); unset, Shop events are a no-op.
  const shopSegmentWriteKey = process.env.SHOP_SEGMENT_WRITE_KEY || process.env.VITE_SHOP_SEGMENT_WRITE_KEY || "";
  const docsBase = (typeof process !== "undefined" && process.env.DOCS_BASE) || "";
  const commitHash = (typeof process !== "undefined" && process.env.COMMIT_HASH) || "";
  const pulseServer = (typeof process !== "undefined" && process.env.PUBLIC_PULSE_SERVER) || "";
  const catalystBase = browserCatalystBase();
  const requestUrl = new URL(request.url);
  const forwardedScheme = request.headers.get("x-forwarded-proto");
  if (forwardedScheme === "http" || forwardedScheme === "https") requestUrl.protocol = forwardedScheme + ":";
  const shopApiMode = serverShopApiMode();
  // A response a shared cache may store names no visitor: the nav reads the account on the client after hydration.
  const shared = context.get(publicResponseContext);
  const wallet = shared ? null : readWallet(request);
  // Only the Creator Hub reads the committee seat: other pages do not ask the node for the curation committee.
  const committee = wallet && isCreatorPath(requestUrl.pathname)
    ? await isCommitteeMember(wallet, request.signal).catch(() => false)
    : false;
  return {
    thirdwebClientId,
    walletConnectProjectId,
    magicApiKey,
    segmentWriteKey,
    segmentApiHost,
    shopSegmentWriteKey,
    docsBase,
    commitHash,
    pulseServer,
    catalystBase,
    publicPageBase: catalystBase || requestUrl.origin,
    shopApiMode,
    wallet,
    locale: shared ? undefined : sitesLanguage(parseCookies(request.headers.get("cookie")).lang).code,
    /** The curation committee seat; read only for a Creator Hub path, false on every other path. */
    committee,
    /**
     * The session id, also on a first visit, for a page whose own loader does not run on a client
     * navigation. Null on a response a shared cache may store (the docs, the cart's shared document).
     */
    sid: pageSid(request, context),
    /**
     * Segment's `userId`: the wallet the server verified (`dcl_wallet_verified`), never the display
     * cookie `wallet` comes from, which the browser writes. Null when none, and on a shared-cacheable response.
     */
    analyticsUserId: shared ? null : readVerifiedWallet(request),
  };
}

/** Root's data on this page came from a response a shared cache may store, so it names no visitor. */
let rootNamesNoVisitor = false;

/** App gives root's session id here: null only on a response a shared cache may store (`pageSid`). */
export function noteRootSid(sid: string | null | undefined): void {
  rootNamesNoVisitor = sid === null;
}

// Case-insensitive, as React Router matches routes.
const CREATOR_PATH = /^\/(create|creator-hub)(\/|$)/i;
function isCreatorPath(pathname: string): boolean {
  return CREATOR_PATH.test(pathname);
}

export function shouldRevalidate({
  currentUrl,
  nextUrl,
  formMethod,
  defaultShouldRevalidate,
}: ShouldRevalidateFunctionArgs) {
  // The verified wallet changed (a sign-in or sign-out posted to /auth/callback): root's data, the
  // page views' userId included, is read again, whatever else this navigation is.
  if (sessionChangePending()) {
    return true;
  }
  if (formMethod) {
    return defaultShouldRevalidate;
  }
  // The Creator Hub framed by the Explorer reloads in place for another account.
  if (reloadingForAccount()) {
    return true;
  }
  if (currentUrl.pathname === nextUrl.pathname) {
    return false;
  }
  // Leaving a publicly cacheable page (the docs) for one that is not: root's data there has no
  // session, which page views need, so it is read again.
  if (isPublicResponsePath(currentUrl.pathname) && !isPublicResponsePath(nextUrl.pathname)) {
    return true;
  }
  // The same after the cart's shared document (shared-cart.ts), also on a move to another Shop page.
  if (rootNamesNoVisitor) {
    return true;
  }
  // Entering the Creator Hub reads the committee seat the page before it did not.
  if (isCreatorPath(nextUrl.pathname) && !isCreatorPath(currentUrl.pathname)) {
    return true;
  }
  // Shop pages read root's data only as a first-render seed, so a move between them waits for no server request.
  if (isShopPath(currentUrl.pathname) && isShopPath(nextUrl.pathname)) {
    return false;
  }
  // Single fetch gives true on every navigation that is not after an action (react-router
  // dom/ssr/single-fetch.js), so a move out of the Shop reads root's data again.
  return defaultShouldRevalidate;
}

/**
 * The shell card for every page without a head of its own, without a canonical: naming the home page
 * there would tell search engines those pages duplicate it. The 404 of an unknown path has no card.
 */
export function meta({ error }: Route.MetaArgs) {
  if (isRouteErrorResponse(error) && error.status === 404) return notFoundMeta();
  return homeMeta(undefined);
}

/** A route's inline script for the document head: it runs while the document parses, before the page's code. */
export type HeadScriptHandle = { headScript?: string };

/**
 * The matched routes' head scripts. Only the document that the server sends runs them: React makes a script that a
 * client render adds through `innerHTML`, so the browser does not run it. Thus a route gives the browser bundle an
 * empty text, and hydration keeps the server's text in the element.
 */
export function RouteHeadScripts() {
  return useMatches().map(({ id, handle }) => {
    const script = (handle as HeadScriptHandle | undefined)?.headScript;
    return script === undefined ? null : (
      // eslint-disable-next-line react/no-danger
      <script key={id} suppressHydrationWarning dangerouslySetInnerHTML={{ __html: script }} />
    );
  });
}

export function Layout({ children }: { children: React.ReactNode }) {
  const data = useRouteLoaderData("root") as
    | { thirdwebClientId?: string; walletConnectProjectId?: string; magicApiKey?: string; segmentWriteKey?: string; segmentApiHost?: string; shopSegmentWriteKey?: string; docsBase?: string; commitHash?: string; pulseServer?: string; catalystBase?: string; publicPageBase?: string; shopApiMode?: string; locale?: SitesLocale }
    | undefined;
  const [selectedLocale, setLocale] = useState<SitesLocale>();
  const homeData = useRouteLoaderData("routes/_index") as { locale?: SitesLocale } | undefined;
  const locale = selectedLocale ?? homeData?.locale ?? data?.locale ?? "en";
  const localized = useMatches().some(({ handle }) => (handle as SitesLocaleHandle | undefined)?.sitesLocalized === true);
  const publicEnv = JSON.stringify({
    thirdwebClientId: data?.thirdwebClientId ?? "",
    walletConnectProjectId: data?.walletConnectProjectId ?? "",
    magicApiKey: data?.magicApiKey ?? "",
    SEGMENT_WRITE_KEY: data?.segmentWriteKey ?? "",
    SEGMENT_API_HOST: data?.segmentApiHost ?? "",
    SHOP_SEGMENT_WRITE_KEY: data?.shopSegmentWriteKey ?? "",
    docsBase: data?.docsBase ?? "",
    commitHash: data?.commitHash ?? "",
    pulseServer: data?.pulseServer ?? "",
  }).replace(/</g, "\\u003c");
  const catalystBase = data?.catalystBase
    ? `;window.__CATALYST_BASE__=${JSON.stringify(data.catalystBase).replace(/</g, "\\u003c")}`
    : "";
  // The Shop client's legacy-alias fallback (ui3/src/shop/api) while the node has not mounted /shop-api.
  const shopApiMode = data?.shopApiMode === "legacy" ? `;window.__SHOP_API_MODE__="legacy"` : "";

  return (
    // The docs theme boot script sets data-docs-theme* on <html> before hydration.
    <html lang={localized ? locale : "en"} suppressHydrationWarning>
      <head>
        <meta charSet="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <script
          // eslint-disable-next-line react/no-danger
          dangerouslySetInnerHTML={{
            // Set Zod's shared configuration before route modules construct schemas.
            // Even its caught eval capability probe produces a CSP violation.
            __html: `globalThis.__zod_globalConfig={...globalThis.__zod_globalConfig,jitless:true};window.__DCL_PUBLIC__=${publicEnv}${catalystBase}${shopApiMode}`,
          }}
        />
        <script
          // eslint-disable-next-line react/no-danger
          dangerouslySetInnerHTML={{ __html: EMBED_BOOT_SCRIPT }}
        />
        <script
          // eslint-disable-next-line react/no-danger
          dangerouslySetInnerHTML={{ __html: NAV_PENDING_SCRIPT }}
        />
        <RouteHeadScripts />
        <link rel="icon" type="image/svg+xml" href={logo} />
        <Meta />
        <Links />
      </head>
      <body>
        <SitesLocaleContext.Provider value={{ locale, onSelect: setLocale }}>
          <ThumbnailPublicBase.Provider value={data?.publicPageBase ?? ""}>
            {children}
          </ThumbnailPublicBase.Provider>
        </SitesLocaleContext.Provider>
        <ScrollRestoration />
        <Scripts />
      </body>
    </html>
  );
}

const CLIENT_ERROR_ENDPOINT = "/internal/client-error";
const CLIENT_ERROR_BUDGET = 10;
const CLIENT_ERROR_DEDUPE_MS = 60_000;

function installClientErrorReporter(): () => void {
  if (typeof window === "undefined") return () => {};
  // A browser under automation (headless probes, CDP-driven checks, test rigs) reports
  // its own environment, not the product, burying the real errors.
  if ((navigator as Navigator & { webdriver?: boolean }).webdriver) return () => {};

  const seen = new Map<string, number>();
  let sent = 0;
  let reporting = false;

  const report = (name: string, message: string, stack: string) => {
    if (reporting || sent >= CLIENT_ERROR_BUDGET) return;
    const now = Date.now();
    const key = `${name}:${message}`;
    const last = seen.get(key);
    if (last !== undefined && now - last < CLIENT_ERROR_DEDUPE_MS) return;
    reporting = true;
    try {
      seen.set(key, now);
      sent += 1;
      void fetch(CLIENT_ERROR_ENDPOINT, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          message,
          name,
          stack,
          url: window.location.href,
          ua: navigator.userAgent,
          ts: new Date(now).toISOString(),
        }),
        keepalive: true,
      }).catch(() => {});
    } catch {
    } finally {
      reporting = false;
    }
  };

  const describe = (value: unknown): string => {
    if (typeof value === "string") return value;
    try {
      return JSON.stringify(value) ?? String(value);
    } catch {
      return String(value);
    }
  };

  const onError = (event: ErrorEvent) => {
    const err = event.error;
    if (err instanceof Error) {
      report(err.name || "Error", err.message || event.message, err.stack ?? "");
      return;
    }
    if (!event.message) return;
    report(
      "Error",
      event.message,
      `${event.filename ?? ""}:${event.lineno ?? 0}:${event.colno ?? 0}`,
    );
  };

  const onRejection = (event: PromiseRejectionEvent) => {
    const reason = event.reason;
    if (reason instanceof Error) {
      report(
        reason.name || "UnhandledRejection",
        reason.message || "(no message)",
        reason.stack ?? "",
      );
      return;
    }
    report("UnhandledRejection", describe(reason) || "(no message)", "");
  };

  const onResourceError = (event: Event) => {
    if (event instanceof ErrorEvent) return;
    const t = event.target;
    if (!(t instanceof Element)) return;
    const el = t as HTMLImageElement & HTMLScriptElement & HTMLLinkElement;
    const src = el.currentSrc || el.src || el.href || "";
    if (!src) return;
    report("ResourceError", `${t.tagName.toLowerCase()} failed to load: ${src}`.slice(0, 500), "");
  };

  const origError = console.error.bind(console);
  const origWarn = console.warn.bind(console);
  console.error = (...args: unknown[]) => {
    origError(...args);
    report("ConsoleError", args.map(describe).join(" ").slice(0, 500), "");
  };
  console.warn = (...args: unknown[]) => {
    origWarn(...args);
    report("ConsoleWarning", args.map(describe).join(" ").slice(0, 500), "");
  };

  window.addEventListener("error", onError);
  window.addEventListener("unhandledrejection", onRejection);
  window.addEventListener("error", onResourceError, true);
  return () => {
    window.removeEventListener("error", onError);
    window.removeEventListener("unhandledrejection", onRejection);
    window.removeEventListener("error", onResourceError, true);
    console.error = origError;
    console.warn = origWarn;
  };
}

let appHydrated = false;

export default function App() {
  // The @ui components read profiles and the like through react-query hooks
  // (ui3/src/data/hooks); one client per request on the server, one per page
  // in the browser, so SSR never shares a cache between visitors.
  const [queryClient] = useState(() => new QueryClient(QUERY_CLIENT_CONFIG));
  useRoutePageView();
  useSessionChangeRevalidation();
  const rootSid = (useRouteLoaderData("root") as { sid?: string | null } | undefined)?.sid;
  useEffect(() => noteRootSid(rootSid), [rootSid]);
  useEffect(() => {
    appHydrated = true;
    document.documentElement.removeAttribute("data-nav-loading");
    if (import.meta.env.DEV) {
      import("@/features/lib/dev/dev-kit").then((m) => m.installDevKit());
    }
    return installClientErrorReporter();
  }, []);
  return (
    <QueryClientProvider client={queryClient}>
      <ChromeAuthBridge>
        <ChromeNavBridge>
          <RoutePending />
          <Outlet />
          <EmbedBridge />
        </ChromeNavBridge>
      </ChromeAuthBridge>
    </QueryClientProvider>
  );
}

export function ErrorBoundary({ error }: Route.ErrorBoundaryProps) {
  const isRouteError = isRouteErrorResponse(error);
  const routeStatus = isRouteError ? error.status : 0;
  useNotFoundPageView(routeStatus === 404);

  let name = "Error";
  let message = "An unexpected error occurred.";
  let stack = "";
  if (error instanceof Error) {
    name = error.name || "Error";
    message = error.message || message;
    stack = error.stack || "";
  } else if (typeof error === "string") {
    message = error;
  } else if (error) {
    try {
      message = JSON.stringify(error);
    } catch {
      message = String(error);
    }
  }

  useEffect(() => {
    // The error page is hydrated too: the pending bar the head script started goes away as App's does.
    document.documentElement.removeAttribute("data-nav-loading");
    if (isRouteError) {
      if (routeStatus === 404 && appHydrated) {
        try {
          void fetch("/internal/page-not-found", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
              path: window.location.pathname + window.location.search,
              referrer: document.referrer,
            }),
            keepalive: true,
          }).catch(() => {});
        } catch {
        }
      }
      return;
    }
    try {
      void fetch(CLIENT_ERROR_ENDPOINT, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          message,
          name,
          stack,
          url: window.location.href,
          ua: navigator.userAgent,
          ts: new Date().toISOString(),
        }),
        keepalive: true,
      }).catch(() => {});
    } catch {
    }
  }, [isRouteError, routeStatus, name, message, stack]);

  return (
    <div lang="en">
      {isRouteError ? <RouteErrorView error={error} /> : <StatusPage status={500} />}
    </div>
  );
}

function RouteErrorView({
  error,
}: {
  error: { status: number; statusText?: string; data?: unknown };
}) {
  // A dead link lands on the page decentraland.org shows, with the 404 status kept.
  if (error.status === 404) return <NotFoundPage />;
  return <StatusPage status={error.status} />;
}
