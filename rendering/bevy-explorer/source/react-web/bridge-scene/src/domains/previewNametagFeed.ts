// The Shop's nametag over its avatar, told to the page (previewNametag): where the engine has the avatar's nametag
// anchor, followed from the engine's stream while a Names page is on the Shop's stage (the engine tells it on subscribe
// and again whenever it moves by a step), and told once it moves; or none, for the page to keep the design's place,
// when the stage comes up again or goes, the avatar goes or changes, or the stream ends. An anchor a stream told after
// it was stopped, or for another avatar, never lands.

import type { WindowPoint } from './previewView'

/** The least move told to the page: about a pixel of the 942px window. */
export const NAMETAG_STEP = 0.001
/** For this long after the tag is placed afresh (the feed starting, another avatar, an emote begun or stopped, a jump
 *  the hold let through), an anchor told places it where it is and is never held against: the engine blends an
 *  animation into the next over up to 0.4s, and a place taken mid-blend would otherwise be kept for as long as the page
 *  is open. The first anchor told after it places the tag too, and that place is the one held. */
export const NAMETAG_SETTLE_SECONDS = 0.75
/** How long after a stream ended (or couldn't open) it is followed again, while the avatar is still on the stage. */
export const NAMETAG_RETRY_SECONDS = 1

export interface NametagFeed<A> {
  /** The Shop's stage came up or went: the stream is stopped, and a page holding a place is told there is none. */
  restage(): void
  /** A frame, `dt` seconds on: `avatar` is the Shop's avatar while a Names page is on its stage, else null. */
  tick(dt: number, avatar: A | null): void
  /** The view moved (the camera easing to another page's framing): the last anchor told is told where it falls now. */
  reproject(): void
  /** The avatar's pose is changing (an emote begun or stopped): the tag stays where it is, and the anchors told over the
   *  next NAMETAG_SETTLE_SECONDS place it afresh; the first told after places the one held. */
  release(): void
}

export function createNametagFeed<A, O>(deps: {
  /** Follows where `avatar` has its nametag anchor: `tell` gets each anchor the engine tells (null while it has none),
   *  and `ended` is called once the stream ends or fails; the function returned stops it. */
  follow: (avatar: A, tell: (anchor: O | null) => void, ended: () => void) => () => void
  /** The anchor in the window, by the view as it is when the anchor is told. */
  project: (anchor: O) => WindowPoint
  send: (anchor: WindowPoint | null) => void
  /** The anchor kept from one told, given the one kept before (none since the last drop, or while settling). */
  settle: (held: O | null, told: O) => O
}): NametagFeed<A> {
  let told: WindowPoint | null = null
  // The last anchor kept for the avatar on the stage: none since the last drop.
  let last: O | null = null
  // The stream followed now, and its token: an anchor told by any other is dropped.
  let stop: (() => void) | null = null
  let stream = 0
  // While no stream is followed for the avatar on the stage (it ended), how long until it is followed again.
  let retry = 0
  // How long anchors told still place the tag afresh rather than holding its place.
  let settling = NAMETAG_SETTLE_SECONDS
  // Whether the place kept came from an anchor told once the pose had settled: only then is it held against.
  let seeded = false
  let lastAvatar: A | null = null
  const publish = (anchor: WindowPoint | null): void => {
    if (anchor == null || !Number.isFinite(anchor.x) || !Number.isFinite(anchor.y)) {
      if (told != null) {
        told = null
        deps.send(null)
      }
      return
    }
    if (told != null && Math.abs(anchor.x - told.x) < NAMETAG_STEP && Math.abs(anchor.y - told.y) < NAMETAG_STEP) return
    told = anchor
    deps.send(anchor)
  }
  const unfollow = (): void => {
    stream++
    stop?.()
    stop = null
  }
  const drop = (): void => {
    unfollow()
    last = null
    settling = NAMETAG_SETTLE_SECONDS
    seeded = false
  }
  const tell = (from: number, anchor: O | null): void => {
    if (from !== stream) return
    if (anchor == null) {
      last = null
      publish(null)
      return
    }
    const settled = settling <= 0
    const held = settled && seeded ? last : null
    const next = deps.settle(held, anchor)
    // A jump the hold let through (a hat loaded, a pose the emote left): the place is taken afresh, and the one held is
    // the first told once that settles.
    if (held != null && next !== held) {
      settling = NAMETAG_SETTLE_SECONDS
      seeded = false
    } else if (settled) seeded = true
    last = next
    publish(deps.project(next))
  }
  return {
    restage(): void {
      drop()
      lastAvatar = null
      publish(null)
    },
    tick(dt: number, avatar: A | null): void {
      settling = Math.max(0, settling - dt)
      if (avatar == null) {
        if (lastAvatar != null) drop()
        lastAvatar = null
        publish(null)
        return
      }
      if (avatar === lastAvatar) {
        if (stop != null) return
        // its stream ended: followed again once the wait is over
        retry -= dt
        if (retry > 0) return
      } else {
        // another avatar (or the first): the last one's place isn't its, until its own stream tells it
        drop()
        lastAvatar = avatar
        publish(null)
      }
      const from = ++stream
      stop = deps.follow(
        avatar,
        (anchor) => tell(from, anchor),
        () => {
          if (from !== stream) return
          // ended: the design's place until it is followed again
          stop = null
          retry = NAMETAG_RETRY_SECONDS
          last = null
          publish(null)
        }
      )
    },
    reproject(): void {
      if (last != null) publish(deps.project(last))
    },
    release(): void {
      settling = NAMETAG_SETTLE_SECONDS
      seeded = false
    }
  }
}

/**
 * Follows an engine stream (`open` subscribes to it), telling each value as `read` makes it an anchor (null for one the
 * engine hasn't measured), and `ended` once the stream ends, fails or can't be opened. The function returned stops it:
 * the stream is closed, and nothing it says after is told.
 */
export function followStream<S, O>(
  open: () => Promise<AsyncIterable<S> & { close?: () => void }>,
  read: (value: S) => O | null,
  tell: (anchor: O | null) => void,
  ended: () => void
): () => void {
  let stopped = false
  let stream: (AsyncIterable<S> & { close?: () => void }) | null = null
  void (async () => {
    try {
      stream = await open()
      if (stopped) {
        stream.close?.()
        return
      }
      for await (const value of stream) {
        if (stopped) return
        tell(read(value))
      }
    } catch {
      // a stream that can't open, or fails: the design's place until another is followed
    }
    if (!stopped) ended()
  })()
  return () => {
    stopped = true
    stream?.close?.()
  }
}
