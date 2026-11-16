import { z } from "zod";

import type { AuthIdentity } from "../../auth/types";
import { CatalystError, postJSON } from "../client";

export const WatcherCredentialsSchema = z.object({
  url: z.string().min(1),
  token: z.string().min(1),
  roomId: z.string().min(1),
  identity: z.string().min(1),
  placeName: z.string().optional(),
});
export type WatcherCredentials = z.infer<typeof WatcherCredentialsSchema>;

/** The gatekeeper's watcher route; the viewer signs this path. */
export const WATCHER_TOKEN_PATH = "/comms-gatekeeper/cast/watcher-token";

/** Upstream's text for a stream credential past its expiry (401). */
const EXPIRED_STREAM_ACCESS = "Stream access has expired";

export type WatchError =
  | "no_active_stream"
  | "access_expired"
  | "banned"
  | "refused"
  | "unavailable";

type WatchStatus = "live" | "waiting" | "expired";

export type WatchResult = {
  status: WatchStatus;
  location: string;
  isWorld: boolean;
  placeName: string;
  identity: string;
  error?: WatchError;
};

export function isWorldLocation(location: string): boolean {
  return !/^-?\d+\s*,\s*-?\d+$/.test(location.trim());
}

/** What a refused watcher-token request means for the viewer. */
export function watchErrorFor(err: unknown): WatchError {
  if (!(err instanceof CatalystError)) return "unavailable";
  if (err.status === 404) return "no_active_stream";
  if (err.status === 403) return "banned";
  if (err.status === 401) {
    return err.message.startsWith(EXPIRED_STREAM_ACCESS) ? "access_expired" : "refused";
  }
  return "unavailable";
}

/** Upstream's watcher contract: the viewer's own signed POST of the location and a display name. */
export async function requestWatcherCredentials(
  location: string,
  displayName: string,
  identity: AuthIdentity,
  opts: { signal?: AbortSignal } = {},
): Promise<WatcherCredentials> {
  const raw = await postJSON<unknown>(
    WATCHER_TOKEN_PATH,
    { location, identity: displayName },
    { identity, signal: opts.signal },
  );
  return WatcherCredentialsSchema.parse(raw);
}
