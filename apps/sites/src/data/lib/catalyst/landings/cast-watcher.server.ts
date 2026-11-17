// The /landings/cast-watch story page: every state is simulated from the `active` intent. The
// real watcher is /cast/w/{location} (features/social/cast-live), which asks the gatekeeper
// for a signed watcher token in the browser.
import { fallbackPlaceName, isWorldLocation, type WatchResult } from "./cast-watcher";

export { fallbackPlaceName };

export type WatchIntent = "live" | "waiting" | "expired";

export async function resolveWatch(
  location: string,
  identity: string,
  intent: WatchIntent,
): Promise<WatchResult> {
  const base = {
    location,
    isWorld: isWorldLocation(location),
    placeName: fallbackPlaceName(location),
    identity,
    fallback: true,
  };
  if (intent === "expired") return { ...base, status: "expired", error: "access_expired" };
  if (intent === "waiting") return { ...base, status: "waiting", error: "no_active_stream" };
  return { ...base, status: "live" };
}
