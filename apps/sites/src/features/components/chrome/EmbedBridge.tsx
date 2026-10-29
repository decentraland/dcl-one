import { useEffect, useRef } from "react";
import { useLocation, useNavigate } from "react-router";

import { embedEscapeHref, framedPathname, installEmbedGuards, isEmbedded } from "@ui/web/frames/embed";

import { explorerHost, hubLocationKeeper, installHubFrame } from "./explorer-host";

export default function EmbedBridge() {
  const location = useLocation();
  const navigate = useNavigate();
  const here = location.pathname + location.search + location.hash;
  const latest = useRef({ navigate, here });
  latest.current = { navigate, here };
  const keeper = useRef<ReturnType<typeof hubLocationKeeper> | null>(null);

  useEffect(() => {
    if (typeof window === "undefined" || !isEmbedded()) return;
    const host = explorerHost();
    if (!host) return installEmbedGuards(window);
    // An Explorer account switch reloads this page's data: navigating to the same URL revalidates
    // it, and root's loader (the wallet and its committee seat) joins while the navigation runs.
    return installHubFrame(window, host, () =>
      latest.current.navigate(latest.current.here, { replace: true, preventScrollReset: true }));
  }, []);

  useEffect(() => {
    if (typeof window === "undefined" || !isEmbedded()) return;
    const host = explorerHost();
    if (!host) {
      const href = embedEscapeHref(location.pathname, location.search, location.hash);
      if (href && framedPathname(window) !== location.pathname) (window.top ?? window).location.assign(href);
      return;
    }
    keeper.current ??= hubLocationKeeper(host);
    const back = keeper.current(location, window.location.href);
    if (back) void navigate(back, { replace: true });
  }, [location, navigate]);

  return null;
}
