import { type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { pageLoader } from "@/core/lib/telemetry/page-loader";

import LiveCastWatch from "@/features/social/cast-live/LiveCastWatch";

import type { Route } from "./+types/cast.w.$location.parcel.$parcel";
import type { StoryId } from "@/core/lib/telemetry/story-id";

const STORY: StoryId = "cast/watch";

const DEFAULT_ASSIGNMENT: PageDefaults = {
  variant: "live",
};

// A world watcher link that names a parcel (upstream cast2 `/cast/w/:worldName/parcel/:parcel`).
export async function loader({ request, context, params }: Route.LoaderArgs) {
  const { sid, assignment, wrap } = await pageLoader(request, context, DEFAULT_ASSIGNMENT);
  return wrap({ sid, assignment, location: params.location.trim(), parcel: params.parcel.trim() });
}

export function meta() {
  return [{ title: "Decentraland Cast" }, { name: "robots", content: "noindex" }];
}

export default function CastWatchParcelPage({ loaderData }: Route.ComponentProps) {
  const { sid, assignment, location, parcel } = loaderData;
  return (
    <main className="cast-watch-route">
      <LiveCastWatch
        key={`${location}/${parcel}`}
        trackCtx={{ sid, story: STORY, variant: assignment.variant }}
        location={location}
        parcel={parcel}
      />
    </main>
  );
}
