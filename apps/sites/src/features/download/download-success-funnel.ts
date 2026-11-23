import { useEffect, useRef, useState, type MouseEvent } from "react";

import { resetDownloadCtaClicked } from "./download-click";
import {
  createDownloadTracker,
  funnelExtra,
  nextRevisit,
  onPageHidden,
  resolveAnonUserId,
  resolveDownloadPlace,
  resolveMacArch,
  sendDownloadFunnelExit,
  sendDownloadPageExit,
  sendDownloadSuccessArrived,
  type FunnelArch,
  type FunnelSession,
  type FunnelPlace,
} from "./download-funnel";
import { deliverInstaller, installerFilename, type Dispatch } from "./installer-delivery";
import {
  chooseInstallerUrl,
  collectDeepLinkParams,
  resolveGatewayAnonUserId,
  resolveReferrer,
  reportedInstallerUrl,
  withInstallerQuery,
} from "./installer-gateway";

export type DownloadSuccessFunnel = FunnelSession & {
  /** The plain installer for this system on the installer host; the download gateway's link replaces it when set. */
  href: string;
  /** The download gateway's base, or null when the deployment configures none. */
  gateway: string | null;
  /** The sign-in token for the gateway's identity route; resolves undefined for a signed-out visitor. */
  identityId: (signal: AbortSignal) => Promise<string | undefined>;
  os: "windows" | "macos";
  arch: FunnelArch;
  /** The page's `?place=`, `?mac_arch=` and `?anon_user_id=`, as given. */
  place: string | null;
  macArch: string | null;
  anonUserId: string | null;
  /** Hands the installer (or its streamed bytes) to the browser. */
  dispatch: Dispatch;
};

/** The transfer overlay: shown from arrival until a download is saved or fails; `progress` null is indeterminate. */
export type DownloadOverlay = { progress: number | null } | null;

type Transfer = { downloading: boolean; progress: number | null; failed: boolean; saved: boolean };

/**
 * /download_success's funnel, as the legacy page ran it: `download_success_arrived` on arrival, then the automatic
 * attempt's `download_started` and `download_success` or `download_failed`, a footer retry as an attempt of its own
 * (`place: download-success-footer`, and no `Click`), and `download_funnel_exit` on every hide once a download
 * control led here (a known `place`). Returns the footer retry's click handler and the transfer overlay, which
 * follows the legacy backdrop: up from arrival, with progress while a stream runs, down once saved or failed.
 */
export function useDownloadSuccessFunnel(input: DownloadSuccessFunnel): {
  retry: (event: MouseEvent<HTMLAnchorElement>) => void;
  overlay: DownloadOverlay;
} {
  const [transfer, setTransfer] = useState<Transfer>({ downloading: false, progress: null, failed: false, saved: false });
  const latest = useRef(input);
  latest.current = input;
  const os = input.os === "windows" ? "Windows" : "macOS";
  const place: FunnelPlace = resolveDownloadPlace(input.place);
  const loadedAt = useRef(Date.now());
  const revisit = useRef<number | null>(null);
  const fired = useRef({ startedFired: false, successFired: false, failedFired: false });
  const mounted = useRef(false);
  const started = useRef<{ href: string; controller: AbortController } | null>(null);
  const attempts = useRef(new Set<AbortController>());
  const retrying = useRef(false);
  // The id the last attempt gave the gateway, which the exit snapshot reports so its row joins that attempt's.
  const gatewayAnonUserId = useRef<string | undefined>(undefined);

  const facts = () => {
    const { sid, userId, anonUserId, arch } = latest.current;
    return { sid, userId, arch, anonUserId: resolveAnonUserId(anonUserId, sid), authenticated: Boolean(userId), revisit: revisit.current ?? 0 };
  };

  // Each attempt owns its cancellation, as the legacy page's AbortController did: aborting it cancels the sign-in
  // token request, the HEAD request and the hold, and the attempt then reports no outcome.
  const attempt = async (attemptPlace: FunnelPlace, controller = new AbortController()) => {
    const { signal } = controller;
    attempts.current.add(controller);
    const { sid, userId, arch, anonUserId, authenticated, revisit: visit } = facts();
    const { href: plain, gateway, identityId, dispatch, macArch } = latest.current;
    const params = new URLSearchParams(window.location.search);
    const deepLink = collectDeepLinkParams(params);
    const referrer = resolveReferrer(params);
    const anon = resolveGatewayAnonUserId(anonUserId, sid, deepLink, referrer);
    gatewayAnonUserId.current = anon;
    // The footer retry reads its attribution as it is clicked; the automatic attempt once its URL is known.
    const early = attemptPlace === "download-success-footer" ? funnelExtra(resolveMacArch(macArch)) : undefined;
    const tracker = (href: string) =>
      createDownloadTracker(
        { href, os, arch, place: attemptPlace, anonUserId: anon, authenticated, revisit: visit, extra: early ?? funnelExtra(resolveMacArch(macArch)) },
        { sid, userId },
      );
    let current: ReturnType<typeof tracker> | null = null;
    const footer = attemptPlace === "download-success-footer";
    // The legacy footer retry kept the earlier outcome; the automatic attempt starts from a clean slate.
    setTransfer((state) => (footer ? { ...state, downloading: true, progress: null } : { downloading: true, progress: null, failed: false, saved: false }));
    try {
      const chosen = await chooseInstallerUrl({ os, arch, fallback: plain, gateway, identityId, anonUserId: anon, signal });
      if (signal.aborted) return;
      const { url } = chosen;
      const href = withInstallerQuery(url, { anon_user_id: anon, ...deepLink, ...(referrer ? { referrer } : {}) });
      current = tracker(reportedInstallerUrl(href, gateway, chosen.identityId));
      fired.current.startedFired = true;
      current.started();
      const filename = installerFilename(url);
      const delivery = await deliverInstaller(href, os, dispatch, signal, {
        filename,
        onProgress: (progress) => setTransfer((state) => ({ ...state, progress })),
      });
      if (signal.aborted) return;
      setTransfer((state) => ({ ...state, progress: 100, saved: footer ? state.saved : true }));
      fired.current.successFired = true;
      current.success(filename, delivery);
    } catch (error) {
      if (signal.aborted) return;
      setTransfer((state) => ({ ...state, failed: true }));
      fired.current.failedFired = true;
      // A failure before the URL was known reports the plain installer, the best link the page had.
      (current ?? tracker(plain)).failed(error instanceof Error ? error.message : "Download failed");
    } finally {
      attempts.current.delete(controller);
      if (!signal.aborted) setTransfer((state) => ({ ...state, downloading: false }));
    }
  };

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      // StrictMode remounts synchronously; only a real unmount is still unmounted once this microtask runs.
      queueMicrotask(() => {
        if (mounted.current) return;
        for (const controller of attempts.current) controller.abort();
      });
    };
  }, []);

  useEffect(() => {
    if (revisit.current !== null) return;
    const { sid, userId, arch } = latest.current;
    revisit.current = nextRevisit(os, arch);
    sendDownloadSuccessArrived({ os, arch, place, revisit: revisit.current, authenticated: Boolean(userId) }, { sid, userId });
    // Once per page: a later locale or query change is the same arrival.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    // Locale changes keep this attempt; a different artifact cancels it and starts another.
    if (started.current?.href === input.href) return;
    started.current?.controller.abort();
    started.current = { href: input.href, controller: new AbortController() };
    void attempt(place, started.current.controller);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [input.href]);

  useEffect(() => {
    if (place === "unknown") return;
    return onPageHidden(() => {
      const { sid, userId, arch, anonUserId, authenticated, revisit: visit } = facts();
      sendDownloadFunnelExit(
        {
          os,
          arch,
          place,
          ...fired.current,
          msOnPage: Date.now() - loadedAt.current,
          revisit: visit,
          authenticated,
          anonUserId: gatewayAnonUserId.current ?? anonUserId,
        },
        { sid, userId },
      );
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [place, os]);

  const retry = (event: MouseEvent<HTMLAnchorElement>) => {
    event.preventDefault();
    if (retrying.current) return;
    retrying.current = true;
    void attempt("download-success-footer").finally(() => {
      retrying.current = false;
    });
  };
  const shown = transfer.downloading || (!transfer.failed && !transfer.saved);
  return { retry, overlay: shown ? { progress: transfer.downloading ? transfer.progress : null } : null };
}

/** /download's `download_page_exit` on every hide, counting from mount; a click made elsewhere does not count. */
export function useDownloadPageExit(session: FunnelSession): void {
  const latest = useRef(session);
  latest.current = session;
  const loadedAt = useRef(Date.now());
  useEffect(() => {
    resetDownloadCtaClicked();
    return onPageHidden(() => sendDownloadPageExit(Date.now() - loadedAt.current, latest.current));
  }, []);
}
