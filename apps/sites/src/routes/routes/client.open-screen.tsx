import { loadPlaces } from "@/data/lib/catalyst/places/index.server";
import { pageLoader } from "@/core/lib/telemetry/page-loader";
import OpenScreen from "@/features/explorer/open-screen/OpenScreen";

import type { Route } from "./+types/client.open-screen";
import type { StoryId } from "@/core/lib/telemetry/story-id";

const STORY: StoryId = "client/open-screen";

export async function loader({ request }: Route.LoaderArgs) {
  const { sid, wrap } = await pageLoader(request,  { variant: "base" });
  const places = await loadPlaces({ limit: 24 })
    .then((result) => result.data)
    .catch(() => null);
  return wrap({ sid, places });
}

export default function ClientOpenScreen({ loaderData }: Route.ComponentProps) {
  const d = loaderData;

  return (
    <main className="client-open-screen-route">
      <OpenScreen
        places={d.places}
        trackCtx={{
          sid: d.sid,
          story: STORY,
          variant: "base",
        }}
      />
    </main>
  );
}
