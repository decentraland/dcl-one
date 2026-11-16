import { isWorldLocation, type WatchResult } from "./cast-watcher";

export type WatchIntent = "live" | "waiting" | "expired";

export function fallbackPlaceName(location: string): string {
  const loc = location.trim();
  if (!loc) return "this Decentraland Cast";
  return isWorldLocation(loc) ? loc : `Parcel ${loc}`;
}

/** The page before the viewer joins. The watcher token needs the viewer's own signature, so
    the browser asks for it on Join (`requestWatcherCredentials`). */
export function resolveWatch(
  location: string,
  identity: string,
  intent: WatchIntent,
): WatchResult {
  const base = {
    location,
    isWorld: isWorldLocation(location),
    placeName: fallbackPlaceName(location),
    identity,
  };
  if (intent === "expired") return { ...base, status: "expired", error: "access_expired" };
  if (intent === "waiting") return { ...base, status: "waiting", error: "no_active_stream" };
  return { ...base, status: "live" };
}
