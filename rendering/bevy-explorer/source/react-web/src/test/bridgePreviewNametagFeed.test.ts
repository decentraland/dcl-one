// The Shop's nametag, as the bridge tells it to the page (bridge-scene/src/domains/previewNametagFeed): followed from the
// engine's stream while a Names page is on the stage and told once it moves; none when the stage comes up again or goes,
// the avatar goes, or the stream ends; an anchor a stopped stream tells never lands; and a place is held only once the
// pose has settled.
import { describe, expect, it } from 'vitest'
import { createNametagFeed, followStream, NAMETAG_RETRY_SECONDS, NAMETAG_SETTLE_SECONDS } from '../../bridge-scene/src/domains/previewNametagFeed'

type Point = { x: number; y: number }

/** One stream the feed followed: its avatar, how to tell it an anchor or end it, and whether it was stopped. */
type Followed = { avatar: number; tell: (p: Point | null) => void; end: () => void; stopped: boolean }

/** A feed over streams the test drives; the anchor is the point itself. `settle` keeps what the test says. */
function feed(settle: (held: Point | null, told: Point) => Point = (_, told) => told, project: (p: Point) => Point = (p) => p) {
  const streams: Followed[] = []
  const sent: (Point | null)[] = []
  const nametag = createNametagFeed<number, Point>({
    follow: (avatar, tell, ended) => {
      const followed: Followed = { avatar, tell, end: ended, stopped: false }
      streams.push(followed)
      return () => {
        followed.stopped = true
      }
    },
    settle,
    project,
    send: (p) => sent.push(p)
  })
  return { nametag, streams, sent }
}

const A = { x: 0.53, y: 0.12 }
const B = { x: 0.47, y: 0.31 }
/** Keeps the place it is given, as the Shop's hold does while the head only sways. */
const holds = (held: Point | null, told: Point): Point => held ?? told

describe("the Shop's nametag feed", () => {
  it('follows the avatar once, tells the page where the anchor is, and that there is none once the stage goes', () => {
    const { nametag, streams, sent } = feed()
    nametag.tick(0, 7)
    nametag.tick(0.5, 7)
    expect(streams).toHaveLength(1)
    streams[0].tell(A)
    expect(sent).toEqual([A])
    nametag.restage() // the stage goes
    nametag.restage()
    expect(sent).toEqual([A, null])
    expect(streams[0].stopped).toBe(true)
  })

  it('never lands what a stopped stream tells, and follows afresh once the stage is up again', () => {
    const { nametag, streams, sent } = feed()
    nametag.tick(0, 7)
    streams[0].tell(A)
    nametag.restage() // closed
    nametag.restage() // and opened again
    streams[0].tell(B) // the old stage's stream, told after it was stopped
    expect(sent).toEqual([A, null])
    nametag.tick(0, 7)
    expect(streams).toHaveLength(2)
    streams[1].tell(B)
    expect(sent).toEqual([A, null, B])
  })

  it('goes back to the design while the engine has no anchor, and to the anchor once it has one', () => {
    const { nametag, streams, sent } = feed()
    nametag.tick(0, 7)
    streams[0].tell(A)
    streams[0].tell(null)
    streams[0].tell(A)
    expect(sent).toEqual([A, null, A])
  })

  it('follows again a while after its stream ended, never every frame', () => {
    const { nametag, streams, sent } = feed()
    nametag.tick(0, 7)
    streams[0].tell(A)
    streams[0].end()
    expect(sent).toEqual([A, null])
    nametag.tick(NAMETAG_RETRY_SECONDS / 2, 7)
    expect(streams).toHaveLength(1)
    nametag.tick(NAMETAG_RETRY_SECONDS / 2, 7)
    expect(streams).toHaveLength(2)
    streams[1].tell(B)
    expect(sent).toEqual([A, null, B])
    // an end the feed no longer follows says nothing
    streams[0].end()
    expect(sent).toEqual([A, null, B])
  })

  it("tells a move only once it passes a pixel, and drops another avatar's stream", () => {
    const { nametag, streams, sent } = feed()
    nametag.tick(0, 7)
    streams[0].tell(A)
    streams[0].tell({ x: A.x, y: A.y + 0.0004 })
    expect(sent).toEqual([A])
    nametag.tick(0, 8) // avatar 8 takes the stage
    expect(streams.map((s) => [s.avatar, s.stopped])).toEqual([
      [7, true],
      [8, false]
    ])
    streams[0].tell(B) // avatar 7's, late
    expect(sent).toEqual([A, null])
    streams[1].tell(B)
    expect(sent).toEqual([A, null, B])
  })

  it('tells the page there is none once the avatar goes, and stops following it', () => {
    const { nametag, streams, sent } = feed()
    nametag.tick(0, 7)
    streams[0].tell(A)
    nametag.tick(0, null)
    expect(sent).toEqual([A, null])
    expect(streams[0].stopped).toBe(true)
  })

  it('follows the view as it moves, from the last anchor told, until the stage goes', () => {
    let lift = 0 // the view's move: a point shows this much higher in the window
    const { nametag, streams, sent } = feed(undefined, (p) => ({ x: p.x, y: p.y - lift }))
    nametag.reproject() // nothing told yet: nothing to tell
    nametag.tick(0, 7)
    streams[0].tell(A)
    lift = 0.05 // the camera eases to Names' framing
    nametag.reproject()
    expect(sent).toEqual([A, { x: A.x, y: A.y - 0.05 }])
    nametag.restage() // the stage goes
    nametag.reproject()
    expect(sent).toEqual([A, { x: A.x, y: A.y - 0.05 }, null])
  })

  it('places the tag where each anchor says until the pose settles, after it starts and after an emote, then holds the first settled place', () => {
    const { nametag, streams, sent } = feed(holds)
    nametag.tick(0, 7)
    streams[0].tell(A)
    streams[0].tell(B) // still settling (a blend into the idle): placed
    expect(sent).toEqual([A, B])
    nametag.tick(NAMETAG_SETTLE_SECONDS, 7)
    streams[0].tell(A) // the first told once settled: placed, and held from now on
    streams[0].tell(B)
    expect(sent).toEqual([A, B, A])
    nametag.release() // an emote: nothing is told until an anchor places the tag afresh
    expect(sent).toEqual([A, B, A])
    streams[0].tell(B)
    expect(sent).toEqual([A, B, A, B])
    nametag.tick(NAMETAG_SETTLE_SECONDS, 7)
    streams[0].tell(A) // settled again: this one is held
    streams[0].tell(B)
    expect(sent).toEqual([A, B, A, B, A])
  })

  it('takes a jump the hold lets through afresh, and holds the first place told once that settles', () => {
    // holds a place unless the anchor moves far (a hat loaded): then the new one, as the Shop's hold does
    const jumps = (held: Point | null, told: Point): Point => (held != null && Math.abs(told.y - held.y) < 0.1 ? held : told)
    const { nametag, streams, sent } = feed(jumps)
    nametag.tick(0, 7) // followed: the window opens with it
    nametag.tick(NAMETAG_SETTLE_SECONDS, 7)
    streams[0].tell(A) // settled: held
    streams[0].tell({ x: A.x, y: A.y + 0.05 }) // a sway: held at A
    expect(sent).toEqual([A])
    streams[0].tell(B) // a jump: placed, and the window opens again
    streams[0].tell({ x: B.x, y: B.y + 0.05 }) // still settling after it: placed where it is
    expect(sent).toEqual([A, B, { x: B.x, y: B.y + 0.05 }])
    nametag.tick(NAMETAG_SETTLE_SECONDS, 7)
    streams[0].tell(B) // the first once settled: placed and held
    streams[0].tell({ x: B.x, y: B.y + 0.05 })
    expect(sent).toEqual([A, B, { x: B.x, y: B.y + 0.05 }, B])
  })

  it("seeds the hold afresh for another avatar, never from the last one's", () => {
    const { nametag, streams, sent } = feed(holds)
    nametag.tick(NAMETAG_SETTLE_SECONDS, 7)
    streams[0].tell(A) // avatar 7's held place
    nametag.tick(0, 8) // avatar 8 takes the stage, mid-blend
    streams[1].tell(A)
    nametag.tick(NAMETAG_SETTLE_SECONDS, 8)
    streams[1].tell(B) // its first once settled: placed, not held against the blend
    expect(sent).toEqual([A, null, A, B])
  })
})

describe('following an engine stream', () => {
  /** A stream the test feeds, and whether it was closed. */
  function stream<S>() {
    const queue: S[] = []
    let wake: (() => void) | null = null
    let done = false
    const state = { closed: false }
    const iterable = {
      close: () => {
        state.closed = true
        done = true
        wake?.()
      },
      async *[Symbol.asyncIterator]() {
        while (true) {
          while (queue.length > 0) yield queue.shift()!
          if (done) return
          await new Promise<void>((resolve) => (wake = resolve))
        }
      }
    }
    return {
      iterable,
      state,
      push: (v: S) => {
        queue.push(v)
        wake?.()
      },
      finish: () => {
        done = true
        wake?.()
      }
    }
  }
  const tick = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0))

  it('tells each value as an anchor, nothing after it is stopped, and closes the stream', async () => {
    const s = stream<{ measured: boolean; y: number }>()
    const told: (number | null)[] = []
    let ended = 0
    const stop = followStream(
      async () => s.iterable,
      (v) => (v.measured ? v.y : null),
      (a) => told.push(a),
      () => ended++
    )
    s.push({ measured: false, y: 0 })
    s.push({ measured: true, y: 3.6 })
    await tick()
    expect(told).toEqual([null, 3.6])
    stop()
    s.push({ measured: true, y: 3.7 })
    await tick()
    expect(told).toEqual([null, 3.6])
    expect(s.state.closed).toBe(true)
    expect(ended).toBe(0)
  })

  it('says it ended when the stream ends, fails or never opens', async () => {
    const s = stream<number>()
    let ended = 0
    followStream(
      async () => s.iterable,
      (v) => v,
      () => {},
      () => ended++
    )
    s.finish()
    await tick()
    expect(ended).toBe(1)
    followStream(
      () => Promise.reject(new Error('no such op')),
      (v: number) => v,
      () => {},
      () => ended++
    )
    await tick()
    expect(ended).toBe(2)
  })

  it('closes a stream that opens after it was stopped', async () => {
    const s = stream<number>()
    let open: (it: typeof s.iterable) => void = () => {}
    const stop = followStream(
      () => new Promise<typeof s.iterable>((resolve) => (open = resolve)),
      (v) => v,
      () => {},
      () => {}
    )
    stop()
    open(s.iterable)
    await tick()
    expect(s.state.closed).toBe(true)
  })
})
