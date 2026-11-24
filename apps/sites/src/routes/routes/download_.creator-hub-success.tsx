import { useEffect, useRef } from "react";

import { DownloadSuccess } from "@dcl/one-ui";
import SitesLayout from "@/features/components/chrome/SitesLayout";

import { CREATOR_HUB_RELEASES, CREATOR_HUB_STEPS } from "@/features/download/creator-hub-steps";
import { trackCreatorHubClick } from "@/features/download/creator-hub-click";
import { useLegacyTrackScope } from "@/core/lib/telemetry/legacy-click";

import { loadLatestRelease, resolveDownload } from "@/data/lib/download/creator-hub.server";
import { sidLoader } from "@/core/lib/telemetry/page-loader";
import { useAnalyticsUserId } from "@/core/lib/telemetry/page-view";
import { track } from "@/core/lib/telemetry/track";

import { publicBase } from "../lib/social-meta";
import { staticPageMeta } from "../lib/static-page-meta";
import type { Route } from "./+types/download_.creator-hub-success";
import type { StoryId } from "@/core/lib/telemetry/story-id";

const STORY: StoryId = "landings/creator-hub-download";

export function meta({ matches }: Route.MetaArgs) {
  return staticPageMeta("/download/creator-hub-success", publicBase(matches));
}

// The steps follow `?os=` (macOS unless it
// says Windows), and the retry link is that system's installer.
export async function loader({ request, context }: Route.LoaderArgs) {
  const url = new URL(request.url);
  const os = url.searchParams.get("os")?.toLowerCase() === "windows" ? "windows" : "macos";
  const resolved = resolveDownload(await loadLatestRelease(), null, { os, arch: url.searchParams.get("arch") });
  const { sid, wrap } = sidLoader(request, context);
  return wrap({ sid, os: resolved.detectedOs, retryUrl: resolved.primary.url });
}

export default function CreatorHubDownloadSuccessRoute({ loaderData }: Route.ComponentProps) {
  const { sid, os, retryUrl } = loaderData;
  const userId = useAnalyticsUserId();
  const scope = useLegacyTrackScope();
  const fired = useRef(false);
  useEffect(() => {
    if (fired.current) return;
    fired.current = true;
    track("lp_creatorhub_download_success", { os }, { sid, story: STORY });
  }, [sid, os]);
  const steps = CREATOR_HUB_STEPS[os === "windows" ? "windows" : "macos"];
  return (
    <SitesLayout active="none" overlayNav>
      <DownloadSuccess
        title="You're almost done!"
        subtitle="Open the file you just downloaded to finish installing Decentraland and start creating"
        stepsLabel="Install steps"
        stepLabel={(n) => `Step ${n}`}
        steps={steps}
        footerLines={[
          {
            text: (link) => <>Trouble with the download? Let&apos;s try that again: {link}</>,
            link: {
              label: "Download Decentraland Creator Hub",
              href: retryUrl || CREATOR_HUB_RELEASES,
              // The legacy footer reported only a retry of a known installer, through `useTrackClick`.
              onClick: retryUrl
                ? () => trackCreatorHubClick("success-footer", { sid, userId, scope }, os === "windows" ? "Windows" : "macOS")
                : undefined,
            },
          },
        ]}
      />
    </SitesLayout>
  );
}
