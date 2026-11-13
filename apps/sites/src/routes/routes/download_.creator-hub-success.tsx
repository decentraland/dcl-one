import { useEffect, useRef } from "react";

import StCreatorHubDownloadSuccess from "@ui/web/pages/StCreatorHubDownloadSuccess";

import { loadDownloadFixture, resolveDownload } from "@/core/lib/landings/creator-hub-download.server";
import { pageLoader, type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { track } from "@/core/lib/telemetry/track";

import type { Route } from "./+types/download_.creator-hub-success";
import type { StoryId } from "@/core/lib/telemetry/story-id";

const STORY: StoryId = "landings/creator-hub-download";

const FALLBACK: PageDefaults = { variant: "os_detected" };

// from: sites@17b3535a:src/pages/download/CreatorHubDownloadSuccess.tsx: the steps follow `?os=` (macOS unless it
// says Windows), and the retry link is that system's installer.
export async function loader({ request, context }: Route.LoaderArgs) {
  const url = new URL(request.url);
  const os = url.searchParams.get("os")?.toLowerCase() === "windows" ? "windows" : "macos";
  const resolved = resolveDownload(loadDownloadFixture(), null, { os, arch: url.searchParams.get("arch") });
  const { sid, wrap } = await pageLoader(request, context, FALLBACK);
  return wrap({ sid, os: resolved.detectedOs, retryUrl: resolved.primary.url });
}

export default function CreatorHubDownloadSuccessRoute({ loaderData }: Route.ComponentProps) {
  const { sid, os, retryUrl } = loaderData;
  const fired = useRef(false);
  useEffect(() => {
    if (fired.current) return;
    fired.current = true;
    track("lp_creatorhub_download_success", { os }, { sid, story: STORY });
  }, [sid, os]);
  return <StCreatorHubDownloadSuccess os={os} retryUrl={retryUrl} />;
}
