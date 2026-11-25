// Which video a Cast page shows. Pure, so it is tested without LiveKit.

/** LiveKit's `Track.Source` values a Cast room carries. */
export type CastSource = "camera" | "microphone" | "screen_share" | "screen_share_audio" | "unknown";

export type StageCandidate = {
  /** The participant's LiveKit identity (`stream:…`, `presentation-bot:…`, a wallet). */
  identity: string;
  source: CastSource;
  muted: boolean;
  local: boolean;
  /** Its media stopped arriving (`STALL_MS` in `session.ts`). */
  stalled: boolean;
  /** A video element of the page shows a frame of it. */
  picture: boolean;
};

/** The identities the gatekeeper mints for Cast publishers (`stream:<place>:<uuid>`,
 * `presentation-bot:<room>:<uuid>`), as the Bevy explorer classifies them. */
export function isCastPublisher(identity: string): boolean {
  return identity.startsWith("stream:") || identity.startsWith("presentation-bot:");
}

/** Who the people badge counts: a watcher (`watch:…`), a wallet in the scene and a streamer
 * (`stream:…`) that publishes. Never the machines the gatekeeper lets in: the presentation bot,
 * the RTMP ingress (`<uuid>-streamer`), the scene's server (`authoritative-server`) and a stream
 * key's connection that publishes nothing. */
export function isPerson(identity: string, publishes: boolean): boolean {
  if (identity === "authoritative-server") return false;
  if (identity.startsWith("presentation-bot:") || identity.endsWith("-streamer")) return false;
  return publishes || !identity.startsWith("stream:");
}

function rank(c: StageCandidate): number {
  const screen = c.source === "screen_share";
  const cast = isCastPublisher(c.identity);
  return screen && cast ? 0 : screen ? 1 : cast ? 2 : 3;
}

/** The video a watcher sees (`shown`) and the one that takes its place with its first picture
 * (`next`, drawn out of sight until then). The track on the stage (`owner`, a key) keeps it while
 * it is published and not muted, stalled or not: a stage that went to another track and back
 * showed no picture each time. Only a track of a better rank that has a picture takes it: a
 * screen share before a camera, a Cast publisher before anyone else in the scene room. Never a
 * muted or non-video track. With no owner, the best rank; ties keep the input order. */
export function watcherStage<T extends StageCandidate & { key: string }>(
  tracks: readonly T[],
  owner: string | null,
): { shown: T | null; next: T | null } {
  let best: T | null = null;
  let held: T | null = null;
  for (const t of tracks) {
    if (t.muted || (t.source !== "camera" && t.source !== "screen_share")) continue;
    if (best === null || rank(t) < rank(best)) best = t;
    if (t.key === owner) held = t;
  }
  if (!best || !held || rank(best) >= rank(held)) return { shown: held ?? best, next: null };
  return best.picture ? { shown: best, next: null } : { shown: held, next: best };
}

/** What a streamer sees of themself: the screen they share, then their camera. */
export function streamerStage<T extends StageCandidate>(tracks: readonly T[]): T[] {
  const own = tracks.filter((t) => t.local && !t.muted);
  return [
    ...own.filter((t) => t.source === "screen_share"),
    ...own.filter((t) => t.source === "camera"),
  ];
}

/** A Cast on air does not arrive: the picture on the watcher's stage (`shown`) stands still or,
 * with no picture, every sound does. */
export function stalledCast(tracks: readonly StageCandidate[], shown: StageCandidate | null): boolean {
  if (shown) return shown.stalled;
  const theirs = tracks.filter((t) => !t.local && !t.muted);
  return theirs.length > 0 && theirs.every((t) => t.stalled);
}
