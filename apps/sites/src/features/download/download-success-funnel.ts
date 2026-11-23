import { useEffect, useRef, type MouseEvent } from "react";

import { resetDownloadCtaClicked, type DownloadClickSession } from "./download-click";
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
  type FunnelPlace,
} from "./download-funnel";
import { deliverInstaller, installerFilename } from "./installer-delivery";

export type DownloadSuccessFunnel = DownloadClickSession & {
  /** The installer this page starts. */
  href: string;
  os: "windows" | "macos";
  arch: FunnelArch;
  /** The page's `?place=`, `?mac_arch=` and `?anon_user_id=`, as given. */
  place: string | null;
  macArch: string | null;
  anonUserId: string | null;
  /** Hands the installer URL to the browser. */
  dispatch: (url: string) => void;
};

/**
 * /download_success's funnel, as the legacy page ran it: `download_success_arrived` on arrival, then the automatic
 * attempt's `download_started` and `download_success` or `download_failed`, a footer retry as an attempt of its own
 * (`place: download-success-footer`, and no `Click`), and `download_funnel_exit` on every hide once a download
 * control led here (a known `place`). Returns the footer retry's click handler.
 */
export function useDownloadSuccessFunnel(input: DownloadSuccessFunnel): (event: MouseEvent<HTMLAnchorElement>) => void {
  const latest = useRef(input);
  latest.current = input;
  const os = input.os === "windows" ? "Windows" : "macOS";
  const place: FunnelPlace = resolveDownloadPlace(input.place);
  const loadedAt = useRef(Date.now());
  const revisit = useRef<number | null>(null);
  const fired = useRef({ startedFired: false, successFired: false, failedFired: false });
  const mounted = useRef(false);
  const started = useRef<string | null>(null);
  const retrying = useRef(false);

  const facts = () => {
    const { sid, userId, anonUserId, arch } = latest.current;
    return { sid, userId, arch, anonUserId: resolveAnonUserId(anonUserId, sid), authenticated: Boolean(userId), revisit: revisit.current ?? 0 };
  };

  const attempt = async (attemptPlace: FunnelPlace, href: string) => {
    const { sid, userId, arch, anonUserId, authenticated, revisit: visit } = facts();
    const extra = funnelExtra(resolveMacArch(latest.current.macArch));
    const tracker = createDownloadTracker(
      { href, os, arch, place: attemptPlace, anonUserId, authenticated, revisit: visit, extra },
      { sid, userId },
    );
    try {
      fired.current.startedFired = true;
      tracker.started();
      const delivery = await deliverInstaller(href, os, latest.current.dispatch);
      // Leaving the page before the hold ends sends no outcome, as the legacy page's abort did.
      if (!mounted.current) return;
      fired.current.successFired = true;
      tracker.success(installerFilename(href), delivery);
    } catch (error) {
      if (!mounted.current) return;
      fired.current.failedFired = true;
      tracker.failed(error instanceof Error ? error.message : "Download failed");
    }
  };

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
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
    // Locale changes keep this attempt; a different artifact starts another.
    if (started.current === input.href) return;
    started.current = input.href;
    void attempt(place, input.href);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [input.href]);

  useEffect(() => {
    if (place === "unknown") return;
    return onPageHidden(() => {
      const { sid, userId, arch, anonUserId, authenticated, revisit: visit } = facts();
      sendDownloadFunnelExit(
        { os, arch, place, ...fired.current, msOnPage: Date.now() - loadedAt.current, revisit: visit, authenticated, anonUserId },
        { sid, userId },
      );
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [place, os]);

  return (event) => {
    event.preventDefault();
    if (retrying.current) return;
    retrying.current = true;
    void attempt("download-success-footer", latest.current.href).finally(() => {
      retrying.current = false;
    });
  };
}

/** /download's `download_page_exit` on every hide, counting from mount; a click made elsewhere does not count. */
export function useDownloadPageExit(session: DownloadClickSession): void {
  const latest = useRef(session);
  latest.current = session;
  const loadedAt = useRef(Date.now());
  useEffect(() => {
    resetDownloadCtaClicked();
    return onPageHidden(() => sendDownloadPageExit(Date.now() - loadedAt.current, latest.current));
  }, []);
}
