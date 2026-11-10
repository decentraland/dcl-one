import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { TrackContext } from "@/core/lib/telemetry/track";
import { isIdentityExpired } from "@/data/lib/auth/expiry";
import { getIdentity } from "@/data/lib/auth/session";

import {
  collectCampaignParams,
  collectDeepLinkParams,
  osLabel,
  parseReferrer,
  resolveMacArch,
  resolvePlace,
  resolvePlatform,
  ANON_USER_ID_PARAM,
  type DownloadPlace,
} from "./params";
import {
  gatewayAnonUserId,
  nextRevisit,
  runDownload,
  type DownloadOutcome,
} from "./success";
import {
  createAttemptTracker,
  readClick,
  trackArrived,
  trackDownloadClick,
  trackFunnelExit,
} from "./telemetry";

export type DownloadSuccessState =
  | { phase: "downloading"; progress: number | null }
  | { phase: "saved" }
  | { phase: "failed"; reason: string };

type Fired = { started: boolean; success: boolean; failed: boolean };

/**
 * The /download_success flow (download contract §1, §3; plan behavior 3): the arrival event, then one
 * automatic download per navigation, an explicit re-download, and the funnel exit on hide.
 */
export function useDownloadSuccess(
  search: string,
  session: TrackContext,
  gateway: string | undefined,
) {
  const request = useMemo(() => {
    const params = new URLSearchParams(search);
    const deepLink = collectDeepLinkParams(params);
    return {
      ...resolvePlatform(params),
      params,
      macArch: resolveMacArch(params),
      place: resolvePlace(params.get("place")),
      campaign: collectCampaignParams(params),
      // The gateway takes the position, realm and referrer as source does (contract §3).
      query: {
        position: deepLink.position,
        realm: deepLink.realm,
        referrer: parseReferrer(params.get("referrer")),
      },
    };
  }, [search]);

  const [state, setState] = useState<DownloadSuccessState>({
    phase: "downloading",
    progress: null,
  });
  const visit = useRef<{
    revisit: number;
    anonUserId: string;
    authed: boolean;
    mountedAt: number;
  } | null>(null);
  const fired = useRef<Fired>({
    started: false,
    success: false,
    failed: false,
  });
  const active = useRef<AbortController | null>(null);
  const pendingAbort = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );

  const start = useCallback(
    (place: DownloadPlace) => {
      const current = visit.current;
      if (!current || active.current) return;
      const controller = new AbortController();
      active.current = controller;
      setState({ phase: "downloading", progress: null });
      const { os, arch, macArch, campaign } = request;
      const identity = getIdentity();
      void runDownload({
        os,
        arch,
        gateway,
        identity: identity && !isIdentityExpired(identity) ? identity : null,
        anonUserId: current.anonUserId,
        query: request.query,
        signal: controller.signal,
        onProgress: (progress) => setState({ phase: "downloading", progress }),
        tracker: (href) => {
          const tracker = createAttemptTracker(
            {
              href,
              os,
              arch,
              authState: current.authed ? "authenticated" : "anonymous",
              revisit: current.revisit,
              place,
              anonUserId: current.anonUserId,
              macArch,
              campaign,
            },
            session,
          );
          return {
            started: () => ((fired.current.started = true), tracker.started()),
            success: (result) => (
              (fired.current.success = true),
              tracker.success(result)
            ),
            failed: (reason) => (
              (fired.current.failed = true),
              tracker.failed(reason)
            ),
          };
        },
      }).then((outcome: DownloadOutcome) => {
        if (active.current === controller) active.current = null;
        if (outcome.status === "success") setState({ phase: "saved" });
        else if (outcome.status === "failed")
          setState({ phase: "failed", reason: outcome.reason });
      });
    },
    [request, gateway, session],
  );

  // One automatic download per navigation. React StrictMode runs this effect twice on the same
  // instance; the abort is deferred so the re-run cancels it instead of starting a second download.
  useEffect(() => {
    clearTimeout(pendingAbort.current);
    if (!visit.current) {
      const identity = getIdentity();
      visit.current = {
        revisit: nextRevisit(request.os, request.arch),
        anonUserId: gatewayAnonUserId(
          request.params.get(ANON_USER_ID_PARAM),
          session.sid,
        ),
        authed: !!identity && !isIdentityExpired(identity),
        mountedAt: Date.now(),
      };
      trackArrived(
        {
          os: request.os,
          arch: request.arch,
          authState: visit.current.authed ? "authenticated" : "anonymous",
          revisit: visit.current.revisit,
          place: request.place,
          campaign: request.campaign,
        },
        session,
      );
      start(request.place);
    }
    return () => {
      pendingAbort.current = setTimeout(() => active.current?.abort(), 0);
    };
  }, [request, session, start]);

  // download_funnel_exit on every hide, only for arrivals from a known place.
  useEffect(() => {
    if (request.place === "unknown") return;
    const onVisibility = () => {
      const current = visit.current;
      if (document.visibilityState !== "hidden" || !current) return;
      const click = readClick();
      trackFunnelExit(
        {
          os: osLabel(request.os),
          arch: request.arch,
          place: request.place,
          download_started_fired: fired.current.started,
          download_success_fired: fired.current.success,
          download_failed_fired: fired.current.failed,
          ms_on_page: Date.now() - current.mountedAt,
          revisit: current.revisit,
          auth_state: current.authed ? "authenticated" : "anonymous",
          anon_user_id: current.anonUserId,
          ...(click ? { click_id: click.click_id } : {}),
        },
        session,
      );
    };
    document.addEventListener("visibilitychange", onVisibility);
    return () => document.removeEventListener("visibilitychange", onVisibility);
  }, [request, session]);

  /** The footer "download again" link: an explicit click, ignored while a download runs. */
  const redownload = useCallback(() => {
    if (active.current) return;
    trackDownloadClick(
      {
        place: "download-success-footer",
        os: osLabel(request.os),
        target: "desktop_installer",
        macArch: request.macArch,
        campaign: request.campaign,
      },
      session,
    );
    start("download-success-footer");
  }, [request, session, start]);

  return { os: request.os, arch: request.arch, state, redownload };
}
