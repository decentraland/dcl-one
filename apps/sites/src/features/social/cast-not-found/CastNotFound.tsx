import StCastNotFound from "@ui/web/pages/StCastNotFound";

import { track, type TrackContext } from "@/core/lib/telemetry/track";

const HOME_HREF = "/";
const DOCS_HREF = "https://docs.decentraland.org/creator/worlds/cast/";

type CastNotFoundProps = {
  trackCtx: TrackContext;
  from: "streamer" | "watcher" | "unknown";
  reason: "missing" | "malformed" | "expired" | "ended";
  onTrack?: typeof track;
};

export default function CastNotFound({
  trackCtx,
  from,
  reason,
  onTrack = track,
}: CastNotFoundProps) {
  return (
    <div className="cast-not-found">
      <StCastNotFound
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
