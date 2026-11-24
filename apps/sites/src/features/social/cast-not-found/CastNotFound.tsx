import type { ComponentProps } from "react";
import { StCastNotFound } from "@dcl/one-ui/apps/sites";

import { track, type TrackContext } from "@/core/lib/telemetry/track";

import SitesLayout from "../../components/chrome/SitesLayout";

const HOME_HREF = "/";
const DOCS_HREF = "https://docs.decentraland.org/creator/worlds/cast/";

type CastNotFoundProps = {
  trackCtx: TrackContext;
  from: "streamer" | "watcher" | "unknown";
  reason: "missing" | "malformed" | "expired" | "ended";
  onTrack?: typeof track;
};

export function CastNotFoundScreen(props: ComponentProps<typeof StCastNotFound>) {
  return (
    <SitesLayout active="legal" overlayNav>
      <StCastNotFound {...props} />
    </SitesLayout>
  );
}

export default function CastNotFound({
  trackCtx,
  from,
  reason,
  onTrack = track,
}: CastNotFoundProps) {
  return (
    <div className="cast-not-found">
      <CastNotFoundScreen
        homeHref={HOME_HREF}
        docsHref={DOCS_HREF}
        onGoHome={() =>
          onTrack("cast_not_found_go_home", { from, reason }, trackCtx)
        }
        onViewDocs={() =>
          onTrack("cast_not_found_view_docs", { from, reason }, trackCtx)
        }
      />
    </div>
  );
}
