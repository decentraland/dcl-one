import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { TrackContext } from "@/core/lib/telemetry/track";

import { launchFlags } from "./launch";
import { macArchHint } from "./mac-arch";
import type { DownloadOption } from "./options";
import {
  collectCampaignParams,
  collectDeepLinkParams,
  downloadSuccessHref,
  osLabel,
  parseAnonUserId,
  parseReferrer,
  ANON_USER_ID_PARAM,
  type DownloadPlace,
} from "./params";
import { gatewayAnonUserId } from "./success";
import { trackDownloadClick } from "./telemetry";

export type StoreTarget = "app_store" | "google_play" | "epic";

/**
 * The download CTAs every Sites surface shares (/download, the home hero, Come Hang Out): an
 * installer option links to /download_success with the visit's attribution, and store or Epic
 * links record their exit. `place` says which surface the click came from.
 */
export function useDownloadCtas(search: string, session: TrackContext) {
  const request = useMemo(() => {
    const params = new URLSearchParams(search);
    return {
      params,
      campaign: collectCampaignParams(params),
      deepLink: collectDeepLinkParams(params),
      referrer: parseReferrer(params.get("referrer")),
      flags: launchFlags(params),
    };
  }, [search]);
  const ctaClicked = useRef(false);
  // The server render uses only what it can know (the link's id or a UUID sid), so hydration
  // matches; the id kept in this browser's storage joins after mount.
  const carriedAnonId = request.params.get(ANON_USER_ID_PARAM);
  const [anonUserId, setAnonUserId] = useState(
    () => parseAnonUserId(carriedAnonId) ?? parseAnonUserId(session.sid),
  );
  useEffect(
    () => setAnonUserId(gatewayAnonUserId(carriedAnonId, session.sid)),
    [carriedAnonId, session.sid],
  );

  /** The /download_success URL an installer option links to; also its no-JS href. */
  const installerHref = useCallback(
    (option: DownloadOption, place: DownloadPlace): string =>
      downloadSuccessHref(option.os, place, {
        arch: option.arch,
        anonUserId,
        referrer: request.referrer,
        deepLink: request.deepLink,
        campaign: request.campaign,
      }),
    [request, anonUserId],
  );

  /**
   * Records the installer Click (and its click→download join); returns where to navigate. The
   * Click names the surface as source does (`clickPlace`, e.g. "Landing Hero"), which can differ
   * from the place on the /download_success URL.
   */
  const chooseInstaller = useCallback(
    (
      option: DownloadOption,
      place: DownloadPlace,
      clickPlace: string = place,
    ): string => {
      ctaClicked.current = true;
      trackDownloadClick(
        {
          place: clickPlace,
          os: osLabel(option.os),
          target: "desktop_installer",
          macArch: macArchHint() ?? undefined,
          campaign: request.campaign,
        },
        session,
      );
      return installerHref(option, place);
    },
    [request, session, installerHref],
  );

  const leaveForStore = useCallback(
    (target: StoreTarget, place: string) => {
      ctaClicked.current = true;
      const os =
        target === "app_store"
          ? "iOS"
          : target === "google_play"
            ? "Android"
            : undefined;
      trackDownloadClick(
        {
          place,
          target,
          ...(os ? { os } : {}),
          macArch: macArchHint() ?? undefined,
          campaign: request.campaign,
        },
        session,
      );
    },
    [request, session],
  );

  return { request, ctaClicked, installerHref, chooseInstaller, leaveForStore };
}
