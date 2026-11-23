import { useCallback, useEffect, useRef, type MouseEvent } from "react";

import type { DownloadOption, OsKey } from "@/data/lib/download/creator-hub.server";
import { track } from "@/core/lib/telemetry/track";
import { useLegacyTrackScope } from "@/core/lib/telemetry/legacy-click";
import type { StoryId } from "@/core/lib/telemetry/story-id";

import { createDownloadTracker, resolveAnonUserId, type FunnelSession } from "./download-funnel";
import {
  trackDownloadClick,
  type DownloadClickOptions,
  type DownloadClickPlace,
  type DownloadClickSession,
} from "./download-click";

export const CREATOR_HUB_DOWNLOAD_STORY: StoryId = "landings/creator-hub-download";

// The steps page opens 3 s after the click, the delay decentraland.org uses: the visitor sees the browser start
// the installer download before the page changes.
const SUCCESS_DELAY_MS = 3000;

export function successPath(os: OsKey, arch: string): string {
  return `/download/creator-hub-success?${new URLSearchParams({ os, arch }).toString()}`;
}

/** The legacy steps page link after a click: the build's system as the legacy option text, its arch, the visitor. */
export function legacySuccessPath(option: DownloadOption, anonUserId: string | undefined): string {
  const params = new URLSearchParams({ os: osText(option), arch: option.arch });
  if (anonUserId) params.set("anon_user_id", anonUserId);
  return `/download/creator-hub-success?${params.toString()}`;
}

/** Where each surface's `download_started` comes from (legacy `useCreatorHubDownload(place)`). */
const STARTED_PLACE = { "download-page": "creator-hub-download-page", "create-hero": "creators-hero" } as const;

/**
 * Where a Creator Hub installer link is, for its legacy `Click`: the download page and its success page used
 * `useTrackClick` (no conversion), the /create hero `useDownloadClick` without the Explorer funnel's correlation.
 */
export const CREATOR_HUB_CLICK_SURFACES = {
  "download-page": { place: "Download", options: { conversion: false, correlate: false } },
  "create-hero": { place: "Creators Hero", options: { correlate: false } },
  "success-footer": { place: "Creator Hub Success Footer", options: { conversion: false, correlate: false } },
} satisfies Record<string, { place: DownloadClickPlace; options: DownloadClickOptions }>;

export type CreatorHubClickSurface = keyof typeof CREATOR_HUB_CLICK_SURFACES;

/** The legacy `Click` of a Creator Hub download link; `os` is the legacy option text ("macOS", "Windows"). */
export function trackCreatorHubClick(
  surface: CreatorHubClickSurface,
  session: DownloadClickSession,
  os?: "macOS" | "Windows",
): void {
  const { place, options } = CREATOR_HUB_CLICK_SURFACES[surface];
  trackDownloadClick(
    { place, event: "Download", download_target: "creator_hub", ...(os ? { os } : {}) },
    session,
    options,
  );
}

/** The click handler of an installer link: reports the download, then opens the install steps for that build. */
export function useCreatorHubDownloadClick(
  { sid, userId }: Omit<DownloadClickSession, "scope">,
  surface: "download-page" | "create-hero",
) {
  const scope = useLegacyTrackScope();
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    const cancel = () => {
      if (timer.current !== null) clearTimeout(timer.current);
      timer.current = null;
    };
    // A page left for another one may come back from the back/forward cache with the timer still pending.
    window.addEventListener("pagehide", cancel);
    return () => {
      window.removeEventListener("pagehide", cancel);
      cancel();
    };
  }, []);

  return useCallback(
    (option: DownloadOption, kind: "primary" | "secondary", event: MouseEvent<HTMLAnchorElement>) => {
      track(
        "lp_creatorhub_download_clicked",
        { os: option.osKey, arch: option.arch, file_name: option.fileName || null, kind },
        { sid, story: CREATOR_HUB_DOWNLOAD_STORY },
      );
      // The legacy main button carried no `data-os`; each other build named its system.
      trackCreatorHubClick(surface, { sid, userId, scope }, kind === "secondary" ? osText(option) : undefined);
      // Before a release is read the link opens the releases page: the legacy hook started nothing without a file.
      if (!option.fileName) return;
      const anonUserId = resolveAnonUserId(new URLSearchParams(window.location.search).get("anon_user_id"), sid);
      try {
        // The legacy `useCreatorHubDownload`: one intent per click, so no revisit; joined to the steps page by
        // `anon_user_id` and `place`, never by the Explorer's `click_id`.
        createDownloadTracker(
          {
            href: option.url,
            os: osText(option),
            arch: option.arch,
            place: STARTED_PLACE[surface],
            anonUserId,
            authenticated: Boolean(userId),
            revisit: 0,
            extra: { download_target: "creator_hub" },
          },
          { sid, userId },
        ).started();
      } catch {
        // Telemetry never breaks a download.
      }
      // A click that opens the file elsewhere (a new tab or window) leaves this page where it is.
      if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
      if (timer.current !== null) clearTimeout(timer.current);
      const next = legacySuccessPath(option, anonUserId);
      timer.current = setTimeout(() => window.location.assign(next), SUCCESS_DELAY_MS);
    },
    [sid, userId, surface, scope],
  );
}

const osText = (option: DownloadOption): "macOS" | "Windows" => (option.osKey === "macos" ? "macOS" : "Windows");

/**
 * The Creator Hub's `download_success` on arrival at its steps page (legacy `CreatorHubDownloadSuccess`): the file
 * was requested on the page before, so reaching this one completes the funnel. Its revisit counts arrivals per build
 * in this tab; it carries no delivery and no duration.
 */
export function sendCreatorHubSuccess({
  href,
  os,
  arch,
  anonUserId,
  sid,
  userId,
}: FunnelSession & { href: string; os: OsKey; arch: "amd64" | "arm64"; anonUserId: string | null }): void {
  try {
    const system = os === "windows" ? "Windows" : "macOS";
    const key = `creatorHubDownloadSuccess:visits:${system}:${arch}`;
    let revisit = 0;
    try {
      revisit = Number(sessionStorage.getItem(key) ?? "0");
      sessionStorage.setItem(key, String(revisit + 1));
    } catch {
      // Blocked storage counts every arrival as the first.
    }
    createDownloadTracker(
      {
        href,
        os: system,
        arch,
        place: "creator-hub-success-page",
        anonUserId: resolveAnonUserId(anonUserId, sid),
        authenticated: Boolean(userId),
        revisit,
        extra: { download_target: "creator_hub" },
      },
      { sid, userId },
    ).success(href.split("/").pop() || "");
  } catch {
    // Telemetry never breaks the page.
  }
}
