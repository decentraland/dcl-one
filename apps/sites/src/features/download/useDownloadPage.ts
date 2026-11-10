import { useCallback, useEffect, useState } from "react";

import type { TrackContext } from "@/core/lib/telemetry/track";

import { launchDesktopApp } from "./launch";
import type { DownloadOption } from "./options";
import { trackPageExit } from "./telemetry";
import { useDownloadCtas, type StoreTarget } from "./useDownloadCtas";

export type { StoreTarget } from "./useDownloadCtas";

const PLACE = "download-page";
// Query values the onboarding links carry that must not leak through the Referer header.
const PERSONAL_PARAMS = ["email", "user"];

/**
 * The /download page actions (download contract §1-§5): the shared download CTAs from this page,
 * Jump In that opens an installed app or reports that none answered, and download_page_exit on
 * every hide.
 */
export function useDownloadPage(
  search: string,
  session: TrackContext,
  mobile: boolean,
) {
  const ctas = useDownloadCtas(search, session);
  const { request, ctaClicked } = ctas;
  const [launcherMissing, setLauncherMissing] = useState(false);

  useEffect(() => {
    const url = new URL(window.location.href);
    if (!PERSONAL_PARAMS.some((key) => url.searchParams.has(key))) return;
    for (const key of PERSONAL_PARAMS) url.searchParams.delete(key);
    window.history.replaceState(window.history.state, "", url.toString());
  }, []);

  // Not fire-once: every hide is a row, as source does.
  useEffect(() => {
    const mountedAt = Date.now();
    const onVisibility = () => {
      if (document.visibilityState !== "hidden") return;
      trackPageExit(
        {
          cta_clicked: ctaClicked.current,
          ms_on_page: Date.now() - mountedAt,
          ...request.campaign,
        },
        session,
      );
    };
    document.addEventListener("visibilitychange", onVisibility);
    return () => document.removeEventListener("visibilitychange", onVisibility);
  }, [request, session, ctaClicked]);

  const jumpIn = useCallback(async () => {
    const launched = await launchDesktopApp({
      ...request.deepLink,
      ...request.flags,
      mobile,
    });
    if (!launched) setLauncherMissing(true);
  }, [request, mobile]);

  return {
    ctas,
    installerHref: (option: DownloadOption) =>
      ctas.installerHref(option, PLACE),
    chooseInstaller: (option: DownloadOption) =>
      ctas.chooseInstaller(option, PLACE),
    leaveForStore: (target: StoreTarget) => ctas.leaveForStore(target, PLACE),
    jumpIn,
    launcherMissing,
    dismissLauncherMissing: () => setLauncherMissing(false),
  };
}
