import { useCallback, useEffect, useRef, type MouseEvent } from "react";

import type { DownloadOption, OsKey } from "@/data/lib/download/creator-hub.server";
import { track } from "@/core/lib/telemetry/track";
import type { StoryId } from "@/core/lib/telemetry/story-id";

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
export function useCreatorHubDownloadClick({ sid, userId }: DownloadClickSession, surface: CreatorHubClickSurface) {
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
      trackCreatorHubClick(surface, { sid, userId }, kind === "secondary" ? osText(option) : undefined);
      // A click that opens the file elsewhere (a new tab or window) leaves this page where it is. Before a release
      // is read the link opens the releases page here, and nothing is downloading yet.
      if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey || !option.fileName) return;
      if (timer.current !== null) clearTimeout(timer.current);
      timer.current = setTimeout(() => window.location.assign(successPath(option.osKey, option.arch)), SUCCESS_DELAY_MS);
    },
    [sid, userId, surface],
  );
}

const osText = (option: DownloadOption): "macOS" | "Windows" => (option.osKey === "macos" ? "macOS" : "Windows");
