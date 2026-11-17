// Which video a Cast page shows. Pure, so it is tested without LiveKit.

/** LiveKit's `Track.Source` values a Cast room carries. */
export type CastSource = "camera" | "microphone" | "screen_share" | "screen_share_audio" | "unknown";

export type StageCandidate = {
  /** The participant's LiveKit identity (`stream:…`, `presentation-bot:…`, a wallet). */
  identity: string;
  source: CastSource;
  muted: boolean;
  local: boolean;
};

/** The identities the gatekeeper mints for Cast publishers (`stream:<place>:<uuid>`,
 * `presentation-bot:<room>:<uuid>`), as the Bevy explorer classifies them. */
export function isCastPublisher(identity: string): boolean {
  return identity.startsWith("stream:") || identity.startsWith("presentation-bot:");
}

function rank(c: StageCandidate): number {
  const screen = c.source === "screen_share";
  const cast = isCastPublisher(c.identity);
  if (screen && cast) return 0;
  if (screen) return 1;
  if (cast) return 2;
  return 3;
}

/** The video a watcher sees: a screen share before a camera, a Cast publisher before anyone
 * else in the scene room, and never a muted or non-video track. Ties keep the input order. */
export function pickWatcherStage<T extends StageCandidate>(tracks: readonly T[]): T | null {
  let best: T | null = null;
  for (const t of tracks) {
    if (t.muted || (t.source !== "camera" && t.source !== "screen_share")) continue;
    if (best === null || rank(t) < rank(best)) best = t;
  }
  return best;
}

/** What a streamer sees of themself: the screen they share, then their camera. */
export function streamerStage<T extends StageCandidate>(tracks: readonly T[]): T[] {
  const own = tracks.filter((t) => t.local && !t.muted);
  return [
    ...own.filter((t) => t.source === "screen_share"),
    ...own.filter((t) => t.source === "camera"),
  ];
}
