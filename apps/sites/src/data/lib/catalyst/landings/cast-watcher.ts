import type { CastCredentials } from "../cast";

const WATCH_ERRORS = ["no_active_stream", "access_expired"] as const;
type WatchError = (typeof WATCH_ERRORS)[number];

type WatchStatus = "live" | "waiting" | "expired";

export type WatchResult = {
  status: WatchStatus;
  location: string;
  isWorld: boolean;
  placeName: string;
  identity: string;
  credentials?: CastCredentials;
  error?: WatchError;
  fallback?: boolean;
};

export function isWorldLocation(location: string): boolean {
  return !/^-?\d+\s*,\s*-?\d+$/.test(location.trim());
}


export function fallbackPlaceName(location: string): string {
  const loc = location.trim();
  if (!loc) return "this Decentraland Cast";
  return isWorldLocation(loc) ? loc : `Parcel ${loc}`;
}
