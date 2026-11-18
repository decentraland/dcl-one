import { useCallback, useEffect, useRef, type MouseEvent } from "react";

import { track } from "@/core/lib/telemetry/track";
import type { StoryId } from "@/core/lib/telemetry/story-id";

import type { DownloadOption, OsKey } from "./creator-hub.server";

export const CREATOR_HUB_DOWNLOAD_STORY: StoryId = "landings/creator-hub-download";

// The steps page opens 3 s after the click, the delay decentraland.org uses: the visitor sees the browser start
// the installer download before the page changes.
const SUCCESS_DELAY_MS = 3000;

export function successPath(os: OsKey, arch: string): string {
  return `/download/creator-hub-success?${new URLSearchParams({ os, arch }).toString()}`;
}

/** The click handler of an installer link: reports the download, then opens the install steps for that build. */
export function useCreatorHubDownloadClick(sid: string) {
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => () => {
    if (timer.current !== null) clearTimeout(timer.current);
  }, []);

  return useCallback(
    (option: DownloadOption, kind: "primary" | "secondary", event: MouseEvent<HTMLAnchorElement>) => {
      track(
        "lp_creatorhub_download_clicked",
        { os: option.osKey, arch: option.arch, file_name: option.fileName || null, kind },
        { sid, story: CREATOR_HUB_DOWNLOAD_STORY },
      );
      // A click that opens the file elsewhere (a new tab or window) leaves this page where it is.
      if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
      if (timer.current !== null) clearTimeout(timer.current);
      timer.current = setTimeout(() => window.location.assign(successPath(option.osKey, option.arch)), SUCCESS_DELAY_MS);
    },
    [sid],
  );
}
