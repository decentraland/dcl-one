import { EVERY, Following, SRC, Track } from '../../../packages/@dcl/sdk/src/cast/machine'
import { Io, open } from '../../../packages/@dcl/sdk/src/cast/runtime'

const CAM: Track = { identity: 'stream:world:1:u', trackSid: SRC, sourceType: 1 }
const BOT: Track = { identity: 'presentation-bot:room:1:u', trackSid: 'livekit-video://bot/TR1', sourceType: 2 }
const STATE = { type: 'presentation:state', id: 'p1', slideCount: 3, slide: { url: 'u', width: 1440, height: 1080 } }
const flush = () => new Promise((done) => setTimeout(done, 0))

/** An explorer made of plain values: the test sets the clock, the answers and the video report. */
function explorer(over: Partial<Io> = {}) {
  const log: unknown[][] = []
  const world = {
    now: 0,
    tracks: [CAM] as Track[],
    video: undefined as { stamp: number; state: number } | undefined,
    answer: { status: 200, body: '{"presenters":["0xabc"]}' },
    frame: () => {},
    log,
    /** Moves the clock, runs one frame and lets the answers arrive. */
    async at(now: number) {
      world.now = now
      world.frame()
      await flush()
    }
  }
  const io: Io = {
    now: () => world.now,
    frame: (fn) => void (world.frame = fn),
    realm: async () => ({ baseUrl: 'https://node.example', preview: false, address: '0xABC' }),
    tracks: async () => world.tracks,
    packets: async () => [{ sender: BOT.identity, data: JSON.stringify(STATE) }],
    fetch: async (...call) => (log.push(['fetch', ...call]), world.answer),
    topic: (on) => void log.push(['topic', on]),
    publish: (data) => void log.push(['publish', data]),
    screen: (entity, src) => void log.push(['screen', entity, src]),
    video: () => world.video,
    box: (entity, to) => (to && log.push(['box', entity, to]), [8, 4.5]),
    ...over
  }
  return { io, world }
}

describe('runtime', () => {
  it('follows the stream for a viewer and writes the source to every screen', async () => {
    const { io, world } = explorer()
    const cast = open('viewer', io)
    const seen: string[] = []
    cast.onChange((s) => seen.push(s.tag))
    cast.show(1)
    expect(cast.state.tag).toBe('connecting')
    await flush()
    cast.show(2)
    expect(world.log).toEqual([
      ['screen', 1, SRC],
      ['screen', 2, SRC]
    ])
    world.video = { stamp: 3, state: 4 }
    await world.at(0)
    expect(cast.state).toMatchObject({ stream: { tag: 'live', source: { kind: 'cast' } }, video: 'playing' })
    // The realm, the stream and the video: a tick or an answer that shows nothing new is not reported.
    expect(seen).toEqual(['following', 'following', 'following'])
    // @ts-expect-error a viewer handle has nothing to send
    void cast.send
  })

  it('ticks only when the reducer asked to be woken, and reports only changes', async () => {
    let reads = 0
    let changes = 0
    const { io, world } = explorer({ tracks: async () => (reads++, []) })
    const cast = open('viewer', io)
    cast.onChange(() => changes++)
    cast.show(1)
    world.video = { stamp: 1, state: 4 }
    await flush()
    for (const now of [0, 1, 500, EVERY.tracks - 1, EVERY.tracks, EVERY.tracks + 1]) await world.at(now)
    expect(reads).toBe(2)
    // The realm and the video report: the clock and the reads' bookkeeping are not a change for a scene.
    expect(changes).toBe(2)
  })

  it('treats a failed read as lost and a failed fetch as an answer without a status', async () => {
    let fail = true
    const { io, world } = explorer({
      tracks: async () => (fail ? Promise.reject(new Error('down')) : [CAM]),
      fetch: () => Promise.reject(new Error('down'))
    })
    const cast = open('owner', io)
    await flush()
    await world.at(0)
    expect(cast.state).toMatchObject({ stream: { tag: 'off' }, error: { code: 'unavailable', of: 'key', status: 0 } })
    fail = false
    await world.at(EVERY.tracks)
    expect(cast.state).toMatchObject({ stream: { tag: 'live' } })
  })

  it('sends commands, runs the topic effects and stops when closed', async () => {
    const { io, world } = explorer()
    world.tracks = [BOT]
    const cast = open('owner', io)
    await flush()
    await world.at(0)
    await world.at(1)
    await world.at(2)
    expect((cast.state as Following).show).toMatchObject({ tag: 'showing', slides: { id: 'p1', count: 3 } })
    cast.send({ do: 'slide', to: 'next' })
    cast.send({ do: 'revoke' })
    cast.close()
    expect(world.log).toEqual([
      ['fetch', 'GET', 'https://node.example/comms-gatekeeper/scene-stream-access', undefined],
      ['topic', true],
      ['fetch', 'GET', 'https://node.example/comms-gatekeeper/cast/presenters', undefined],
      ['publish', '{"type":"presentation:get-state"}'],
      ['publish', '{"type":"presentation:navigate","action":"next"}'],
      ['fetch', 'DELETE', 'https://node.example/comms-gatekeeper/scene-stream-access', undefined],
      ['topic', false]
    ])
    await world.at(9e9)
    expect(cast.state).toEqual({ tag: 'closed' })
    expect(world.log).toHaveLength(7)
  })

  it('sizes a screen inside its box to the picture, and leaves alone a screen that says so', async () => {
    const { io, world } = explorer()
    world.tracks = [{ ...CAM, width: 400, height: 300 }]
    const cast = open('viewer', io)
    cast.show(1)
    cast.show(2, false)
    await flush()
    await world.at(0)
    cast.show(3)
    world.tracks = [CAM]
    await world.at(EVERY.tracks)
    const sized = world.log.filter(([what]) => what === 'box')
    expect(sized).toEqual([
      ['box', 1, [6, 4.5]],
      ['box', 3, [6, 4.5]],
      ['box', 1, [8, 4.5]],
      ['box', 3, [8, 4.5]]
    ])
  })
})
