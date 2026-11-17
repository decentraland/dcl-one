// Decentraland Cast 2.0 against the comms gatekeeper, as upstream's cast2 web app calls it
// (decentraland/cast2 `src/utils/api.ts`), with the watcher token signed by the viewer's wallet
// as decentraland/comms-gatekeeper b42d45c requires (`authWatcher` on `POST /cast/watcher-token`).
// Browser only: the paths are the gatekeeper's root aliases on the page origin.
import { z } from "zod";

import type { AuthIdentity } from "../auth/types";
import { CatalystError, catalystBase, errorFromResponse, postJSON } from "./client";

export const StreamInfoSchema = z.object({
  placeName: z.string(),
  placeId: z.string(),
  location: z.string(),
  isWorld: z.boolean(),
});
export type StreamInfo = z.infer<typeof StreamInfoSchema>;

export const CastCredentialsSchema = z.object({
  url: z.string().min(1),
  token: z.string().min(1),
  roomId: z.string().min(1),
  identity: z.string().optional(),
  placeName: z.string().optional(),
});
export type CastCredentials = z.infer<typeof CastCredentialsSchema>;

type FetchOpts = { base?: string; signal?: AbortSignal; fetchImpl?: typeof fetch };

// Not `getJSON`: the page shows the gatekeeper's own refusal (`{"error": "Invalid streaming
// key"}`), which `getJSON` replaces with the bare status.
async function readJSON<T>(
  url: string,
  init: RequestInit,
  schema: z.ZodType<T>,
  doFetch: typeof fetch,
): Promise<T> {
  let res: Response;
  try {
    res = await doFetch(url, init);
  } catch (err) {
    throw new CatalystError(`Cast request failed: ${(err as Error)?.message ?? "network error"}`, url);
  }
  if (!res.ok) throw await errorFromResponse(res, url);
  let body: unknown;
  try {
    body = await res.json();
  } catch (err) {
    throw new CatalystError(
      `Cast returned invalid JSON: ${(err as Error)?.message ?? "parse error"}`,
      url,
      res.status,
    );
  }
  return schema.parse(body);
}

/** `GET /cast/stream-info/{streamingKey}`: the place a stream link casts to. Unauthenticated. */
export function getStreamInfo(streamingKey: string, opts: FetchOpts = {}): Promise<StreamInfo> {
  const url = `${catalystBase(opts.base)}/cast/stream-info/${encodeURIComponent(streamingKey)}`;
  return readJSON(
    url,
    { method: "GET", headers: { accept: "application/json" }, signal: opts.signal },
    StreamInfoSchema,
    opts.fetchImpl ?? fetch,
  );
}

/** `POST /cast/streamer-token {token, identity}`: the streaming key is the credential. */
export function getStreamerToken(
  streamingKey: string,
  displayName: string,
  opts: FetchOpts = {},
): Promise<CastCredentials> {
  const url = `${catalystBase(opts.base)}/cast/streamer-token`;
  return readJSON(
    url,
    {
      method: "POST",
      headers: { accept: "application/json", "content-type": "application/json" },
      body: JSON.stringify({ token: streamingKey, identity: displayName }),
      signal: opts.signal,
    },
    CastCredentialsSchema,
    opts.fetchImpl ?? fetch,
  );
}

export type WatcherRequest = {
  /** A parcel (`20,-4`) or a world name (`name.dcl.eth`). */
  location: string;
  /** The display name the stream shows for this viewer. */
  displayName: string;
  /** A world parcel; the gatekeeper accepts it, a world has one stream. */
  parcel?: string;
};

/** `POST /cast/watcher-token`, signed by the viewer so platform and scene bans apply. */
export async function getWatcherToken(
  identity: AuthIdentity,
  req: WatcherRequest,
  opts: { base?: string; signal?: AbortSignal } = {},
): Promise<CastCredentials> {
  const body: Record<string, string> = { location: req.location, identity: req.displayName };
  if (req.parcel) body.parcel = req.parcel;
  const raw = await postJSON<unknown>("/cast/watcher-token", body, {
    identity,
    base: opts.base,
    signal: opts.signal,
  });
  return CastCredentialsSchema.parse(raw);
}

/** A display name for a viewer or streamer who typed none. */
export function defaultDisplayName(address?: string | null): string {
  if (!address) return "Guest";
  return `Guest#${address.slice(-4).toLowerCase()}`;
}
