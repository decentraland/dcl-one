// The Shop's nametag, as the bridge tells it to the page (bridge-scene/src/domains/previewNametagFeed): read while
// the stage is up and told once it moves; none when the stage comes up again or goes, the avatar goes, or a read
// fails; an answer from before never lands on the stage after; and a place is held only once the pose has settled.
import { describe, expect, it } from 'vitest'
import { createNametagFeed, NAMETAG_READ_SECONDS, NAMETAG_SETTLE_SECONDS } from '../../bridge-scene/src/domains/previewNametagFeed'

type Point = { x: number; y: number }

/** A feed whose reads wait to be answered, in the order they were made; the anchor is the point itself. */
function feed() {
  const reads: { avatar: number; answer: (p: Point | null) => void; fail: () => void }[] = []
  const sent: (Point | null)[] = []
  const nametag = createNametagFeed<number, Point>({
    read: (avatar) =>
      new Promise<Point | null>((resolve, reject) => {
        reads.push({ avatar, answer: resolve, fail: () => reject(new Error('read failed')) })
      }),
    project: (p) => p,
    send: (p) => sent.push(p)
  })
  return { nametag, reads, sent }
}

const settle = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0))
const A = { x: 0.53, y: 0.12 }
const B = { x: 0.47, y: 0.31 }

describe("the Shop's nametag feed", () => {
  it('tells the page where the anchor is, and that there is none once the stage goes', async () => {
    const { nametag, reads, sent } = feed()
    nametag.tick(0, 7)
    reads[0].answer(A)
    await settle()
    expect(sent).toEqual([A])
    nametag.restage() // the stage goes
    nametag.restage()
    expect(sent).toEqual([A, null])
  })

  it('drops an answer begun before the stage came up again, for the same avatar', async () => {
    const { nametag, reads, sent } = feed()
    nametag.tick(0, 7)
    reads[0].answer(A)
    await settle()
    nametag.tick(NAMETAG_READ_SECONDS, 7) // a second read, pending
    expect(reads).toHaveLength(2)
    nametag.restage() // closed
    nametag.restage() // and opened again
    reads[1].answer(B) // the old stage's answer
    await settle()
    expect(sent).toEqual([A, null])
    nametag.tick(0, 7)
    reads[2].answer(B)
    await settle()
    expect(sent).toEqual([A, null, B])
  })

  it('goes back to the design while reads fail, and to the anchor once one answers', async () => {
    const { nametag, reads, sent } = feed()
    nametag.tick(0, 7)
    reads[0].answer(A)
    await settle()
    nametag.tick(NAMETAG_READ_SECONDS, 7)
    reads[1].fail()
    await settle()
    nametag.tick(NAMETAG_READ_SECONDS, 7)
    reads[2].answer(A)
    await settle()
    expect(sent).toEqual([A, null, A])
  })

  it("tells a move only once it passes a pixel, and drops another avatar's answer", async () => {
    const { nametag, reads, sent } = feed()
    nametag.tick(0, 7)
    reads[0].answer(A)
    await settle()
    nametag.tick(NAMETAG_READ_SECONDS, 7)
    reads[1].answer({ x: A.x, y: A.y + 0.0004 })
    await settle()
    expect(sent).toEqual([A])
    nametag.tick(NAMETAG_READ_SECONDS, 7) // pending for avatar 7...
    nametag.tick(0, 8) // ...when avatar 8 takes the stage
    reads[2].answer(B)
    await settle()
    expect(reads.map((r) => r.avatar)).toEqual([7, 7, 7, 8])
    expect(sent).toEqual([A, null])
  })

  it("leaves the last avatar's place as soon as another takes the stage, until the new one's read answers", async () => {
    const { nametag, reads, sent } = feed()
    nametag.tick(0, 7)
    reads[0].answer(A)
    await settle()
    nametag.tick(0, 8) // avatar 8's read is held
    expect(sent).toEqual([A, null])
    reads[1].answer(B)
    await settle()
    expect(sent).toEqual([A, null, B])
  })

  it('tells the page there is none once the avatar goes, and reads only every so often', async () => {
    const { nametag, reads, sent } = feed()
    nametag.tick(0, 7)
    reads[0].answer(A)
    await settle()
    nametag.tick(NAMETAG_READ_SECONDS / 2, 7)
    expect(reads).toHaveLength(1)
    nametag.tick(0, null)
    expect(sent).toEqual([A, null])
  })

  it('tells what it keeps of each read, from what it kept before, and keeps nothing over a restage or a failed read', async () => {
    const reads: { answer: (p: Point | null) => void; fail: () => void }[] = []
    const sent: (Point | null)[] = []
    const held: (Point | null)[] = []
    const nametag = createNametagFeed<number, Point>({
      read: () => new Promise<Point | null>((resolve, reject) => reads.push({ answer: resolve, fail: () => reject(new Error('read failed')) })),
      // keeps the first place it is given
      settle: (kept, read) => {
        held.push(kept)
        return kept ?? read
      },
      project: (p) => p,
      send: (p) => sent.push(p)
    })
    // the first read once the pose has settled, so the next is held against it
    nametag.tick(NAMETAG_SETTLE_SECONDS, 7)
    reads[0].answer(A)
    await settle()
    nametag.tick(NAMETAG_READ_SECONDS, 7)
    reads[1].answer(B)
    await settle()
    expect(sent).toEqual([A])
    nametag.restage()
    nametag.restage()
    nametag.tick(0, 7)
    reads[2].answer(B)
    await settle()
    nametag.tick(NAMETAG_READ_SECONDS, 7)
    reads[3].fail()
    await settle()
    nametag.tick(NAMETAG_READ_SECONDS, 7)
    reads[4].answer(A)
    await settle()
    expect(sent).toEqual([A, null, B, null, A])
    expect(held).toEqual([null, A, null, null])
  })

  it('follows the view as it moves, from the last anchor read and without reading again, until the stage goes', async () => {
    const reads: ((p: Point | null) => void)[] = []
    const sent: (Point | null)[] = []
    let lift = 0 // the view's move: a point shows this much higher in the window
    const nametag = createNametagFeed<number, Point>({
      read: () => new Promise<Point | null>((resolve) => reads.push(resolve)),
      project: (p) => ({ x: p.x, y: p.y - lift }),
      send: (p) => sent.push(p)
    })
    nametag.reproject() // nothing read yet: nothing to tell
    nametag.tick(0, 7)
    reads[0](A)
    await settle()
    lift = 0.05 // the camera eases to Names' framing
    nametag.reproject()
    expect(sent).toEqual([A, { x: A.x, y: A.y - 0.05 }])
    expect(reads).toHaveLength(1)
    nametag.restage() // the stage goes
    nametag.reproject()
    expect(sent).toEqual([A, { x: A.x, y: A.y - 0.05 }, null])
  })

  it('places the tag where each read says until the pose settles, after it starts and after an emote, then holds the first settled place', async () => {
    const reads: ((p: Point | null) => void)[] = []
    const sent: (Point | null)[] = []
    const nametag = createNametagFeed<number, Point>({
      read: () => new Promise<Point | null>((resolve) => reads.push(resolve)),
      // keeps the place it is given, as the Shop's hold does while the head only sways
      settle: (kept, read) => kept ?? read,
      project: (p) => p,
      send: (p) => sent.push(p)
    })
    const read = async (dt: number, p: Point): Promise<void> => {
      nametag.tick(dt, 7)
      reads[reads.length - 1](p)
      await settle()
    }
    await read(0, A)
    await read(NAMETAG_READ_SECONDS, B) // still settling (a blend into the idle): the read places it
    expect(sent).toEqual([A, B])
    await read(NAMETAG_SETTLE_SECONDS, A) // the first read begun once settled: placed, and held from now on
    await read(NAMETAG_READ_SECONDS, B)
    expect(sent).toEqual([A, B, A])
    nametag.release() // an emote: nothing is told until a read places the tag afresh
    expect(sent).toEqual([A, B, A])
    await read(NAMETAG_READ_SECONDS, B)
    expect(sent).toEqual([A, B, A, B])
    await read(NAMETAG_SETTLE_SECONDS, A) // settled again: this one is held
    await read(NAMETAG_READ_SECONDS, B)
    expect(sent).toEqual([A, B, A, B, A])
    expect(reads).toHaveLength(7)
  })

  it('never holds a place read before the pose settled, however late it answers', async () => {
    const reads: ((p: Point | null) => void)[] = []
    const sent: (Point | null)[] = []
    const nametag = createNametagFeed<number, Point>({
      read: () => new Promise<Point | null>((resolve) => reads.push(resolve)),
      settle: (kept, read) => kept ?? read,
      project: (p) => p,
      send: (p) => sent.push(p)
    })
    nametag.tick(0, 7) // begun mid-blend...
    nametag.tick(NAMETAG_SETTLE_SECONDS + 0.05, 7)
    reads[0](A) // ...answering once the pose has settled: placed, but not held
    await settle()
    nametag.tick(NAMETAG_READ_SECONDS, 7) // the first read begun once settled
    reads[1](B)
    await settle()
    nametag.tick(NAMETAG_READ_SECONDS, 7)
    reads[2](A) // held at B
    await settle()
    expect(sent).toEqual([A, B])
  })

  it('lands a read pending over a release as it would have, without holding it after', async () => {
    const reads: ((p: Point | null) => void)[] = []
    const sent: (Point | null)[] = []
    const nametag = createNametagFeed<number, Point>({
      read: () => new Promise<Point | null>((resolve) => reads.push(resolve)),
      settle: (kept, read) => kept ?? read,
      project: (p) => p,
      send: (p) => sent.push(p)
    })
    nametag.tick(NAMETAG_SETTLE_SECONDS, 7)
    reads[0](A) // settled: held from now on
    await settle()
    nametag.tick(NAMETAG_READ_SECONDS, 7) // a held read, pending...
    nametag.release() // ...over an emote: it lands held
    reads[1](B)
    await settle()
    expect(sent).toEqual([A])
    nametag.tick(NAMETAG_READ_SECONDS, 7) // begun while settling...
    nametag.tick(NAMETAG_SETTLE_SECONDS, 7) // ...answering once settled: placed afresh
    reads[2](B)
    await settle()
    expect(sent).toEqual([A, B])
    nametag.tick(NAMETAG_READ_SECONDS, 7) // the first read begun once settled again: placed, then held
    reads[3](A)
    await settle()
    nametag.tick(NAMETAG_READ_SECONDS, 7)
    reads[4](B)
    await settle()
    expect(sent).toEqual([A, B, A])
  })

  it("seeds the hold afresh for another avatar, never from the last one's", async () => {
    const reads: ((p: Point | null) => void)[] = []
    const sent: (Point | null)[] = []
    const nametag = createNametagFeed<number, Point>({
      read: () => new Promise<Point | null>((resolve) => reads.push(resolve)),
      settle: (kept, read) => kept ?? read,
      project: (p) => p,
      send: (p) => sent.push(p)
    })
    nametag.tick(NAMETAG_SETTLE_SECONDS, 7)
    reads[0](A) // avatar 7's held place
    await settle()
    nametag.tick(0, 8) // avatar 8 takes the stage, mid-blend
    reads[1](A)
    await settle()
    nametag.tick(NAMETAG_SETTLE_SECONDS, 8) // its first read begun once settled: placed, not held against the blend
    reads[2](B)
    await settle()
    expect(sent).toEqual([A, null, A, B])
  })
})
