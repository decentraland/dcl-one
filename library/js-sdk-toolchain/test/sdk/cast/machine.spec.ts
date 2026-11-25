import {
  BASE,
  CAP,
  Command,
  contain,
  Effect,
  Event,
  EVERY,
  Fetched,
  Following,
  init,
  ratio,
  Read,
  Role,
  serviceUrl,
  SOON,
  SRC,
  State,
  step,
  Track,
  WRITE
} from '../../../packages/@dcl/sdk/src/cast/machine'

const NODE = 'https://node.example'
const GK = `${NODE}/comms-gatekeeper`
const ME = '0xabc'
const ROLES: Role[] = ['viewer', 'admin', 'owner']
const GET_STATE = '{"type":"presentation:get-state"}'
const BOT: Track = { identity: 'presentation-bot:room:1:u', trackSid: 'livekit-video://bot/TR1', sourceType: 2 }
const CAM: Track = { identity: 'stream:world:1:u', trackSid: SRC, sourceType: 1 }
const SLIDES = { type: 'presentation:state', id: 'p1', fileName: 'a.pdf', currentSlide: 2, slideCount: 9 }
// The state the bot sends when clients compose: the slide's picture and the state of its video.
const COMPOSED = { ...SLIDES, videoState: 'paused', slide: { url: 'https://x/s.png', width: 1440, height: 1080 } }

type Out = [State, Effect[]]
const go = (s: State, ...events: Event[]): Out => events.reduce<Out>(([at], e) => step(at, e), [s, []])
const realm = { tag: 'realm', baseUrl: NODE, preview: false, address: '0xABC' } as const
const start = (role: Role): Following => go(init(role, 0)[0], realm)[0] as Following
const answer = (seq: number, of: Fetched, status: number, body: unknown = ''): Event => ({
  tag: 'answer',
  seq,
  of,
  status,
  body: typeof body === 'string' ? body : JSON.stringify(body)
})
const key = (k = 'k1', ends_at = 10 * SOON) => ({ streaming_url: 'rtmp://x', streaming_key: k, ends_at })
const seqOf = (fx: Effect[], of: string) =>
  (fx.find((f) => (f.tag === 'fetch' ? f.of : f.tag) === of) as { seq: number } | undefined)?.seq ?? -1
/** Ticks at the next time `of` is due: the state and the request's number. */
function due(s: State, of: Read): [State, number] {
  const f = s as Following
  const [next, fx] = go(s, { tag: 'tick', now: Math.max(f.now, f.sync[of].at) })
  return [next, seqOf(fx, of)]
}
function poll(s: State, of: 'key' | 'presenters', status: number, body?: unknown): Following {
  const [next, seq] = due(s, of)
  return go(next, answer(seq, of, status, body))[0] as Following
}
function tracks(s: State, ...list: Track[]): Following {
  const [next, seq] = due(s, 'tracks')
  return go(next, { tag: 'tracks', seq, tracks: list })[0] as Following
}
function packets(s: State, list: unknown[], sender = BOT.identity): Following {
  const [next, seq] = due(s, 'packets')
  const sent = list.map((p) => ({ sender, data: JSON.stringify(p) }))
  return go(next, { tag: 'packets', seq, packets: sent })[0] as Following
}
/** Sends a command and applies the answer to its fetch. */
function write(s: State, cmd: Command, status: number, body?: unknown): Out {
  const [sent, fx] = go(s, { tag: 'command', cmd })
  const f = fx.find((x) => x.tag === 'fetch') as Extract<Effect, { tag: 'fetch' }>
  return go(sent, answer(f.seq, f.of, status, body))
}
/** An admin that sees the bot, is a presenter and has asked for the slide state. */
const presenting = (role: Role = 'admin') => poll(tracks(start(role), BOT), 'presenters', 200, { presenters: [ME] })
/** The next tracks snapshot, with the effects of its answer. */
function snapshot(s: State, ...list: Track[]): [Following, Effect[]] {
  const [next, seq] = due(s, 'tracks')
  return go(next, { tag: 'tracks', seq, tracks: list }) as [Following, Effect[]]
}

describe('connection', () => {
  it('reads the realm first, and again with backoff while no answer comes', () => {
    let [s, fx] = init('viewer', 5)
    expect(fx).toEqual([{ tag: 'realm' }, { tag: 'wake', at: 5 + BASE }])
    let now = 5
    for (let delay = BASE; delay < CAP; delay *= 2) {
      expect(go(s, { tag: 'tick', now: now + delay - 1 })).toEqual([s, []])
      now += delay
      ;[s, fx] = go(s, { tag: 'tick', now })
      expect(fx).toEqual([{ tag: 'realm' }, { tag: 'wake', at: now + Math.min(delay * 2, CAP) }])
    }
  })

  it('follows on a realm, with the stream source for the screens', () => {
    const [s, fx] = go(init('owner', 0)[0], realm)
    expect(s).toMatchObject({ tag: 'following', me: ME, base: NODE, key: { tag: 'unknown' }, stream: { tag: 'off' } })
    expect(fx).toEqual([
      { tag: 'screen', src: SRC },
      { tag: 'wake', at: 0 }
    ])
    expect(start('viewer').key).toEqual({ tag: 'hidden' })
  })

  it('is unavailable for good in a preview', () => {
    const [s, fx] = go(init('owner', 0)[0], { ...realm, preview: true })
    expect(s).toMatchObject({ tag: 'unavailable', error: { code: 'preview', retry: { tag: 'never' } } })
    expect(fx).toEqual([])
    expect(go(s, { tag: 'tick', now: 9e9 }, realm)).toEqual([s, []])
  })

  it('closes from every state, leaves the topic, and stays closed', () => {
    for (const s of [init('admin', 0)[0], start('admin'), go(init('admin', 0)[0], { ...realm, preview: true })[0]])
      expect(go(s, { tag: 'close' })).toEqual([{ tag: 'closed' }, []])
    expect(go(presenting(), { tag: 'close' })).toEqual([{ tag: 'closed' }, [{ tag: 'topic', on: false }]])
    expect(go({ tag: 'closed' }, realm, { tag: 'tick', now: 1 }, { tag: 'close' })).toEqual([{ tag: 'closed' }, []])
  })
})

it.each([
  ['https://peer.decentraland.org/content', 'https://comms-gatekeeper.decentraland.org/x'],
  ['https://worlds-content-server.decentraland.zone/world/a.dcl.eth', 'https://comms-gatekeeper.decentraland.zone/x'],
  ['https://realm.decentraland.today:8443', 'https://comms-gatekeeper.decentraland.today/x'],
  ['https://node.example:8443/worlds-content-server/world/a.dcl.eth', 'https://node.example:8443/comms-gatekeeper/x'],
  ['https://notdecentraland.org', 'https://notdecentraland.org/comms-gatekeeper/x'],
  ['', 'https://comms-gatekeeper.decentraland.org/x']
])('serviceUrl(%s) is %s', (base, url) => expect(serviceUrl(base, 'comms-gatekeeper', '/x')).toBe(url))

describe('reads', () => {
  const fetched = (of: Fetched, path: string) => ({ tag: 'fetch', of, method: 'GET', url: GK + path })
  it.each<[Role, object[]]>([
    ['viewer', [{ tag: 'tracks', seq: 1 }]],
    ['admin', [{ tag: 'tracks', seq: 1 }]],
    ['owner', [{ tag: 'tracks', seq: 1 }, fetched('key', '/scene-stream-access')]]
  ])('a %s reads only what its role may, each on its own period', (role, reads) => {
    const [s, fx] = go(start(role), { tag: 'tick', now: 0 })
    expect(fx).toMatchObject([...reads, { tag: 'wake', at: EVERY.tracks }])
    expect(go(s, { tag: 'tick', now: EVERY.tracks - 1 })[1]).toEqual([])
    expect(go(s, { tag: 'tick', now: EVERY.tracks })[1]).toMatchObject([{ tag: 'tracks' }, { tag: 'wake' }])
  })

  it('reads presenters only while a stream is live, and packets only while subscribed', () => {
    const live = go(tracks(start('admin'), CAM), { tag: 'tick', now: 5_000 })[1]
    expect(live).toMatchObject([{ tag: 'tracks' }, fetched('presenters', '/cast/presenters'), { tag: 'wake' }])
    const [, fx] = go(presenting(), { tag: 'tick', now: 5_000 })
    expect(fx.map((f) => f.tag)).toEqual(['tracks', 'packets', 'wake'])
  })

  it('ignores a tick from the past', () => {
    const s = go(start('viewer'), { tag: 'tick', now: 5_000 })[0]
    expect(go(s, { tag: 'tick', now: 4_000 })).toEqual([s, []])
  })
})

describe('stream', () => {
  const user = (trackSid: string): Track => ({ identity: '0xuser', trackSid, sourceType: 1 })
  it.each<[string, Track[], object, string]>([
    ['nothing', [], { tag: 'off' }, 'none'],
    ['a player camera Bevy never shows', [user('livekit-video://0xuser/TR')], { tag: 'off' }, 'none'],
    [
      'the track Unity falls back to',
      [user(SRC)],
      { tag: 'live', source: { identity: '0xuser', kind: 'other' } },
      'none'
    ],
    ['an RTMP ingress', [{ ...CAM, identity: 'k-streamer' }], { tag: 'live', source: { kind: 'rtmp' } }, 'none'],
    ['a Cast streamer', [CAM], { tag: 'live', source: { identity: CAM.identity, kind: 'cast' } }, 'none'],
    ['a screen share first', [CAM, { ...CAM, sourceType: 2 }], { tag: 'live', source: { kind: 'screen' } }, 'none'],
    ['the bot first', [CAM, BOT], { tag: 'live', source: { identity: BOT.identity, kind: 'presentation' } }, 'detected']
  ])('with %s', (_, list, stream, show) => {
    expect(tracks(start('viewer'), ...list)).toMatchObject({ stream, show: { tag: show } })
  })

  const VIDEO = ['loading', 'error', 'loading', 'loading', 'playing', 'stalled', 'stalled', 'paused', 'loading']
  it.each(VIDEO.map((video, state) => [state, video] as const))(
    'reports video state %i as %s, and drops an older report',
    (state, video) => {
      const [s] = go(start('viewer'), { tag: 'video', stamp: 7, state })
      expect(s).toMatchObject({ video, stamp: 7 })
      expect(go(s, { tag: 'video', stamp: 7, state: 4 }, { tag: 'video', stamp: 3, state: 1 })).toEqual([s, []])
    }
  )
})

describe('stream key', () => {
  const none: [number, unknown] = [404, { error: 'No active streaming access found for place' }]
  const owner = () => start('owner')
  const polls = (...answers: [number, unknown][]) => answers.reduce((s, a) => poll(s, 'key', ...a), owner())
  it.each<[string, [number, unknown][], object]>([
    ['none', [none], { tag: 'none' }],
    ['active', [[200, key()]], { tag: 'active', key: { key: 'k1', url: 'rtmp://x' } }],
    ['expiring inside the last day', [[200, key('k1', SOON)]], { tag: 'expiring' }],
    ['expired by the server', [[404, 'Streaming access has expired']], { tag: 'expired' }],
    ['revoked elsewhere', [[200, key()], none], { tag: 'revoked' }],
    ['still revoked', [[200, key()], none, none], { tag: 'revoked' }],
    [
      'replaced elsewhere',
      [
        [200, key()],
        [200, key('k2')]
      ],
      { tag: 'replaced', key: { key: 'k2' } }
    ],
    [
      'still replaced',
      [
        [200, key()],
        [200, key('k2')],
        [200, key('k2')]
      ],
      { tag: 'replaced' }
    ],
    [
      'the same key again',
      [
        [200, key()],
        [200, key()]
      ],
      { tag: 'active' }
    ]
  ])('%s', (_, answers, expected) => expect(polls(...answers).key).toMatchObject(expected))

  it('expires at its end and not before, by the clock alone', () => {
    const end = 2 * SOON
    const s = polls([200, key('k1', end)])
    expect(s.key.tag).toBe('active')
    expect(go(s, { tag: 'tick', now: end - SOON - 1 })[0]).toMatchObject({ key: { tag: 'active' } })
    expect(go(s, { tag: 'tick', now: end - SOON })[0]).toMatchObject({ key: { tag: 'expiring' } })
    expect(go(s, { tag: 'tick', now: end - 1 })[0]).toMatchObject({ key: { tag: 'expiring' } })
    expect((go(s, { tag: 'tick', now: end })[0] as Following).key).toEqual({ tag: 'expired' })
  })

  it.each<[string, unknown]>([
    ['no expiry', { streaming_key: 'k' }],
    ['no key', { ends_at: 5 }],
    ['not JSON', '<html>']
  ])('never holds a key with %s', (_, body) => {
    expect(polls([200, body])).toMatchObject({ key: { tag: 'unknown' }, error: { code: 'invalid', of: 'key' } })
  })

  it.each<[Command['do'], number, unknown, object]>([
    ['generate', 200, key(), { tag: 'active', key: { key: 'k1' } }],
    ['reset', 200, key('k2'), { tag: 'active', key: { key: 'k2' } }],
    ['revoke', 204, '', { tag: 'revoked' }],
    [
      'link',
      200,
      { streamLink: 's', watcherLink: 'w', streamingKey: 'k1', expiresAt: '2100-01-01T00:00:00.000Z' },
      { tag: 'active', key: { key: 'k1', url: 'rtmp://x', links: { stream: 's', watch: 'w' }, endsAt: 4102444800000 } }
    ]
  ])('%s by this owner sets the key', (verb, status, body, expected) => {
    const [s] = write(polls([200, key()]), { do: verb } as Command, status, body)
    expect(s).toMatchObject({ key: expected, write: undefined })
  })
})

describe('errors', () => {
  const live = { error: 'A broadcast is still live.' }
  it.each<[number, unknown, string, boolean, string]>([
    [0, '', 'unavailable', true, ''],
    [429, 'slow down', 'unavailable', true, 'slow down'],
    [503, { ok: false, message: 'Internal error' }, 'unavailable', true, 'Internal error'],
    [409, { error: 'Please try again.' }, 'unavailable', true, 'Please try again.'],
    [409, live, 'live', false, live.error],
    [404, { error: 'place not found for this scene' }, 'pending', true, 'place not found for this scene'],
    [401, 'Expired signature', 'forbidden', false, 'Expired signature'],
    [403, { error: 'not authorized' }, 'forbidden', false, 'not authorized'],
    [400, { error: 'Invalid chain format' }, 'rejected', false, 'Invalid chain format'],
    [200, {}, 'invalid', true, '{}']
  ])('a %i answer is %s', (status, body, code, again, message) => {
    // A poll always reads again: sooner when the refusal can pass, at the cap when it cannot.
    const polled = poll(start('owner'), 'key', status, body)
    const at = again ? Math.max(EVERY.key, BASE) : CAP
    expect(polled.error).toEqual({ code, of: 'key', status, message, retry: { tag: 'at', ms: at } })
    expect(polled.sync.key).toMatchObject({ at, tries: 1 })
    // A write is not repeated: the error tells the caller if and when it may.
    const [written, fx] = write(start('owner'), { do: 'generate' }, status, body)
    const retry = again ? { tag: 'at', ms: BASE } : { tag: 'never' }
    expect(written).toMatchObject({ error: { code, of: 'generate', status, retry }, write: undefined })
    expect(fx.filter((f) => f.tag === 'fetch')).toEqual([])
  })

  it('backs off a failing poll up to the cap, and a good answer clears it', () => {
    let s = start('owner')
    const delays = []
    for (let i = 0; i < 12; i++) {
      const before = s.sync.key.at
      s = poll(s, 'key', 500)
      delays.push(s.sync.key.at - before)
    }
    expect(delays.slice(0, 7)).toEqual([EVERY.key, EVERY.key, EVERY.key, EVERY.key, EVERY.key, 64_000, 128_000])
    expect(Math.max(...delays)).toBe(CAP)
    s = poll(s, 'key', 200, key('k1', 9e15))
    expect(s).toMatchObject({ error: undefined, sync: { key: { tries: 0 } } })
  })
})

describe('commands', () => {
  const cast = `${GK}/cast`
  const fetched = (method: string, url: string, body?: string) => [{ tag: 'fetch', method, url, body }]
  const said = (packet: object) => [{ tag: 'publish', data: JSON.stringify(packet) }]
  const rows: [Command, Role, object[]][] = [
    [{ do: 'promote', who: '0xDEF' }, 'admin', fetched('PUT', `${cast}/presenters/0xdef`)],
    [{ do: 'demote', who: '0xDEF' }, 'admin', fetched('DELETE', `${cast}/presenters/0xdef`)],
    [{ do: 'slide', to: 'next' }, 'admin', said({ type: 'presentation:navigate', action: 'next' })],
    [{ do: 'slide', to: 'prev' }, 'admin', said({ type: 'presentation:navigate', action: 'prev' })],
    [{ do: 'slide', to: 4 }, 'admin', said({ type: 'presentation:navigate', action: 'goto', slideIndex: 4 })],
    [{ do: 'play', video: 1 }, 'admin', said({ type: 'presentation:video:play', videoIndex: 1 })],
    [{ do: 'pause' }, 'admin', said({ type: 'presentation:video:pause' })],
    [{ do: 'stop' }, 'admin', said({ type: 'presentation:stop' })],
    [{ do: 'generate' }, 'owner', fetched('POST', `${GK}/scene-stream-access`)],
    [{ do: 'reset' }, 'owner', fetched('PUT', `${GK}/scene-stream-access`)],
    [{ do: 'revoke' }, 'owner', fetched('DELETE', `${GK}/scene-stream-access`)],
    [{ do: 'link' }, 'owner', fetched('GET', `${cast}/generate-stream-link`)],
    [
      { do: 'present', file: 'https://f/a.pdf' },
      'owner',
      fetched('POST', `${cast}/presentation-bot-token`, '{"streamingKey":"k1"}')
    ]
  ]
  const cases = rows.flatMap((row) => ROLES.map((role) => [role, ...row] as const))
  it.each(cases)('a %s sending %j needs %s', (role, cmd, needs, effects) => {
    const ready = role === 'owner' ? poll(presenting(role), 'key', 200, key()) : presenting(role)
    const [s, fx] = go(ready, { tag: 'command', cmd })
    if (ROLES.indexOf(role) >= ROLES.indexOf(needs)) return expect(fx).toMatchObject(effects)
    expect(fx).toEqual([])
    expect(s).toMatchObject({ error: { code: 'forbidden', of: cmd.do, retry: { tag: 'never' } } })
  })

  it('says nothing to a presentation that is not there', () => {
    const s = start('admin')
    expect(go(s, { tag: 'command', cmd: { do: 'slide', to: 'next' } })).toEqual([s, []])
  })

  it('runs one write at a time and ends one that gets no answer', () => {
    const [busy, sent] = go(start('owner'), { tag: 'command', cmd: { do: 'reset' } })
    const [refused, fx] = go(busy, { tag: 'command', cmd: { do: 'revoke' } })
    expect(fx).toEqual([])
    expect(refused).toMatchObject({ error: { code: 'busy', of: 'revoke', retry: { tag: 'at', ms: WRITE } } })
    const [late] = go(refused, { tag: 'tick', now: WRITE })
    expect(late).toMatchObject({ error: { code: 'timeout', of: 'reset', retry: { tag: 'never' } }, write: undefined })
    expect(go(late, answer(seqOf(sent, 'reset'), 'reset', 200, key()))).toEqual([late, []])
  })

  it('changes the presenters it wrote', () => {
    const [added] = write(presenting(), { do: 'promote', who: '0xDEF' }, 200)
    expect(added).toMatchObject({ presenters: [ME, '0xdef'] })
    expect(write(added, { do: 'promote', who: '0xdef' }, 200)[0]).toMatchObject({ presenters: [ME, '0xdef'] })
    const [left] = write(added, { do: 'demote', who: ME }, 200)
    expect(left).toMatchObject({ presenters: ['0xdef'], topic: true })
    // The bot answers presenters only: a wallet that is one again asks again.
    expect(write(left, { do: 'promote', who: ME }, 200)[1]).toEqual([{ tag: 'publish', data: GET_STATE }])
  })

  it('starts a presentation in two calls, and only with a key', () => {
    const cmd: Command = { do: 'present', file: 'https://f/a.pdf' }
    expect(go(start('owner'), { tag: 'command', cmd })[0]).toMatchObject({ error: { code: 'noKey', of: 'present' } })
    const held = poll(start('owner'), 'key', 200, key())
    const [s, fx] = write(held, cmd, 200, { url: 'wss://lk', token: 't', roomId: 'r' })
    const body = JSON.stringify({ url: cmd.file, livekitToken: 't', livekitUrl: 'wss://lk' })
    expect(fx).toMatchObject([
      { tag: 'fetch', of: 'slides', method: 'POST', url: `${NODE}/cast-presenter-service/presentations`, body }
    ])
    expect(go(s, answer(seqOf(fx, 'slides'), 'slides', 201, { id: 'p1' }))[0]).toMatchObject({ write: undefined })
    expect(write(held, cmd, 200, { url: 'wss://lk' })[0]).toMatchObject({ error: { code: 'invalid', of: 'present' } })
    expect(go(s, answer(seqOf(fx, 'slides'), 'slides', 429, 'full'))[0]).toMatchObject({
      error: { code: 'unavailable', of: 'slides', message: 'full' }
    })
  })
})

describe('presentation', () => {
  it('a viewer listens to the bot, and sends nothing', () => {
    const [s, fx] = snapshot(start('viewer'), BOT)
    expect(s).toMatchObject({ show: { tag: 'detected' }, topic: true })
    expect(fx).toMatchObject([{ tag: 'topic', on: true }, { tag: 'wake' }])
    expect(go(s, { tag: 'tick', now: 9e5 })[1].map((f) => f.tag)).toEqual(['tracks', 'packets', 'wake'])
    expect(packets(s, [SLIDES]).show).toMatchObject({ tag: 'showing', slides: { slide: 2, count: 9 } })
  })

  it('an admin becomes a presenter, then asks for the slide state', () => {
    const [read, seq] = due(tracks(start('admin'), BOT), 'presenters') as [Following, number]
    const [s, promote] = go(read, answer(seq, 'presenters', 200, { presenters: ['0xDEF'] }))
    expect(promote).toMatchObject([{ tag: 'fetch', of: 'promote', method: 'PUT', url: `${GK}/cast/presenters/${ME}` }])
    const [joined, asked] = go(s, answer(seqOf(promote, 'promote'), 'promote', 200))
    expect(read.topic).toBe(true)
    expect(joined).toMatchObject({ presenters: ['0xdef', ME], topic: true })
    expect(asked).toEqual([{ tag: 'publish', data: GET_STATE }])
  })

  it('has no presenters while the room does not exist', () => {
    const s = poll(presenting(), 'presenters', 404, { error: 'No active stream found for r' })
    expect([s.presenters, s.error]).toEqual([[], undefined])
  })

  it('follows the slides, drops them when it stops, and ends with the bot', () => {
    const shown = packets(presenting(), [
      'x',
      { type: 'other' },
      { ...SLIDES, id: 7 },
      { ...SLIDES, slideVideos: [{}] }
    ])
    expect(shown.show).toEqual({
      tag: 'showing',
      slides: { id: 'p1', file: 'a.pdf', slide: 2, count: 9, videos: 1, video: 'idle' }
    })
    expect(packets(shown, [{ type: 'presentation:stopped', id: 'p1' }]).show).toEqual({ tag: 'detected' })
    const [read, seq] = due(shown, 'tracks')
    const [ended, left] = go(read, { tag: 'tracks', seq, tracks: [CAM] })
    expect(ended).toMatchObject({ show: { tag: 'none' }, topic: false, stream: { source: { kind: 'cast' } } })
    expect(left).toContainEqual({ tag: 'topic', on: false })
    expect(packets({ ...(ended as Following), topic: true }, [SLIDES]).show).toEqual({ tag: 'none' })
  })

  it('ignores the stop of a replaced presentation', () => {
    const shown = packets(presenting(), [{ ...SLIDES, id: 'old' }, SLIDES, { type: 'presentation:stopped', id: 'old' }])
    expect(shown.show).toMatchObject({ tag: 'showing', slides: { id: 'p1' } })
    expect(packets(shown, [{ type: 'presentation:stopped', id: 'old' }]).show).toEqual(shown.show)
    expect(packets(shown, [{ type: 'presentation:stopped' }]).show).toEqual({ tag: 'detected' })
  })

  it('takes the slide state only from the presentation bot', () => {
    // Any wallet in the room can publish on the topic; the bot's identity comes from the server.
    const forged = packets(presenting(), [SLIDES], '0x00000000000000000000000000000000000000ff')
    expect(forged.show).toEqual({ tag: 'detected' })
    const shown = packets(forged, [SLIDES])
    expect(shown.show.tag).toBe('showing')
    expect(packets(shown, [{ type: 'presentation:stopped' }, { ...SLIDES, currentSlide: 5 }], ME).show).toEqual(
      shown.show
    )
  })

  it.each<[string, object, object]>([
    ['the slide video', { videoState: 'playing' }, { video: 'playing' }],
    ['no slide video for a state it does not know', { videoState: 'seeking' }, { video: 'idle' }],
    ['the size of the slide', {}, { size: { width: 1440, height: 1080 } }],
    ['no size from a slide without one', { slide: { url: 'u', width: 0, height: 1080 } }, { size: undefined }]
  ])('reports %s', (_, sent, slides) => {
    const { show } = packets(tracks(start('viewer'), BOT), [{ ...COMPOSED, ...sent }])
    expect(show).toMatchObject({ tag: 'showing', slides: { video: 'paused', ...slides } })
  })

  it('forgets the presenters when the stream ends: they are read only while it is live', () => {
    const live = poll(tracks(start('owner'), CAM), 'presenters', 200, { presenters: ['stream:w:1:u'] })
    expect(live.presenters).toEqual(['stream:w:1:u'])
    expect(tracks(live).presenters).toEqual([])
  })
})

describe('screen shape', () => {
  const sized = (t: Track, width: number, height: number): Track => ({ ...t, width, height })

  it('is the shape of the slide, else of the track where the client lists its size, else none', () => {
    const [cast, fx] = snapshot(start('viewer'), sized(CAM, 1280, 720))
    expect(cast.stream).toMatchObject({ source: { size: { width: 1280, height: 720 } } })
    expect([ratio(cast), fx]).toEqual([16 / 9, expect.arrayContaining([{ tag: 'fit', ratio: 16 / 9 }])])
    expect(snapshot(cast, sized(CAM, 1280, 720))[1]).not.toContainEqual(expect.objectContaining({ tag: 'fit' }))
    expect(snapshot(cast, CAM)[1]).toContainEqual({ tag: 'fit', ratio: 0 })
    expect(snapshot(cast)[1]).toContainEqual({ tag: 'fit', ratio: 0 })

    const bot = tracks(cast, sized(BOT, 1920, 1080), CAM)
    const [due1, seq] = due(bot, 'packets')
    const [slide, fitted] = go(due1, {
      tag: 'packets',
      seq,
      packets: [{ sender: BOT.identity, data: JSON.stringify(COMPOSED) }]
    })
    expect([ratio(slide as Following), fitted]).toEqual([4 / 3, expect.arrayContaining([{ tag: 'fit', ratio: 4 / 3 }])])
    expect(ratio(packets(slide, [{ type: 'presentation:stopped' }]))).toBe(16 / 9)
    expect(ratio(start('viewer'))).toBe(0)
  })

  it.each([
    [[8, 4.5], 4 / 3, [6, 4.5]],
    [[8, 4.5], 2, [8, 4]],
    [[8, 4.5], 16 / 9, [8, 4.5]],
    [[8, 4.5], 0, [8, 4.5]],
    [[8, 4.5], NaN, [8, 4.5]]
  ] as const)('keeps a picture inside the box %j: ratio %d gives %j', (box, r, size) =>
    expect(contain(box, r)).toEqual(size)
  )
})

// The invariants of the reducer's model, run on the reducer itself over random event orders:
// answers are lost, duplicated, reordered and invented, the clock jumps and commands ignore the role.
describe('invariants', () => {
  const random = (seed: number) => () => {
    seed = (seed + 0x6d2b79f5) | 0
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed)
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
  const COMMANDS: Command[] = [
    { do: 'promote', who: ME },
    { do: 'demote', who: ME },
    { do: 'slide', to: 'next' },
    { do: 'play', video: 0 },
    { do: 'pause' },
    { do: 'stop' },
    { do: 'generate' },
    { do: 'reset' },
    { do: 'revoke' },
    { do: 'link' },
    { do: 'present', file: 'f' }
  ]
  const OWNER_ONLY = ['key', 'generate', 'reset', 'revoke', 'link', 'present', 'slides']
  const READ_OF = (f: Effect): Read | undefined =>
    f.tag === 'tracks' || f.tag === 'packets'
      ? f.tag
      : f.tag === 'fetch' && (f.of === 'key' || f.of === 'presenters')
        ? f.of
        : undefined

  function trace(seed: number, role: Role, steps: number): { s: State; now: number; wake: number } {
    const rnd = random(seed)
    const pick = <T>(list: readonly T[]): T => list[Math.floor(rnd() * list.length)]
    const asked: Effect[] = []
    const last: Partial<Record<Read, number>> = {}
    let [s, fx] = init(role, 0)
    let now = 0
    let wake = 0
    const bodies = () => [
      key(pick(['k1', 'k2']), now + pick([1, SOON, 3 * SOON])),
      { presenters: pick([[], [ME], ['0xDEF', ME]]) },
      { url: 'wss://lk', token: 't' },
      { error: pick(['place not found for this scene', 'Streaming access has expired', 'No active', 'try again']) },
      '<html>'
    ]
    const next = (): Event => {
      const f = pick(asked)
      const roll = rnd()
      if (roll < 0.3)
        return { tag: 'tick', now: (now = Math.max(0, now + pick([-500, 0, 1, 250, 1_000, 30_000, SOON]))) }
      if (roll < 0.4) return { tag: 'video', stamp: Math.floor(rnd() * 20), state: Math.floor(rnd() * 9) }
      if (roll < 0.5) return { tag: 'command', cmd: pick(COMMANDS) }
      if (roll < 0.5005) return pick<Event>([{ ...realm, preview: true }, { tag: 'close' }])
      if (roll < 0.52) return realm
      if (roll < 0.57)
        return answer(Math.floor(rnd() * 40), pick(OWNER_ONLY) as Fetched, pick([200, 404]), pick(bodies()))
      if (!f || f.tag === 'realm') return realm
      if (rnd() < 0.7) asked.splice(asked.indexOf(f), 1)
      if (f.tag === 'tracks') return { tag: 'tracks', seq: f.seq, tracks: pick([[], [CAM], [BOT], [CAM, BOT]]) }
      if (f.tag === 'packets')
        return {
          tag: 'packets',
          seq: f.seq,
          packets: [
            {
              sender: pick([BOT.identity, ME]),
              data: JSON.stringify(pick([SLIDES, COMPOSED, { type: 'presentation:stopped' }]))
            }
          ]
        }
      return f.tag === 'fetch' ? answer(f.seq, f.of, pick([200, 200, 204, 404, 409, 500, 0]), pick(bodies())) : realm
    }
    for (let i = 0; i < steps; i++) {
      asked.push(...fx.filter((f) => f.tag === 'realm' || 'seq' in f))
      const e = next()
      const before = s
      ;[s, fx] = step(s, e)
      wake = fx.reduce((at, f) => (f.tag === 'wake' ? f.at : at), wake)
      const again = step(s, e)

      // Closed and unavailable are final.
      if (before.tag === 'closed' || before.tag === 'unavailable') expect([s, fx]).toEqual([before, []])
      // A duplicated event changes nothing more; only a command acts twice.
      if (e.tag !== 'command' && e.tag !== 'tick') expect(again).toEqual([s, []])
      if (e.tag === 'tick') expect([again[0], again[1].filter((f) => f.tag !== 'wake')]).toEqual([s, []])
      if (s.tag !== 'following') continue
      const t = s.now
      if (before.tag === 'following') {
        // The clock and the request counter never go back.
        expect(t).toBeGreaterThanOrEqual(before.now)
        expect(s.seq).toBeGreaterThanOrEqual(before.seq)
        // An answer that is stale, repeated or never asked for is ignored.
        const y = 'seq' in e && e.tag !== 'answer' ? before.sync[e.tag].done : -1
        if ('seq' in e && (e.seq <= y || e.seq > before.seq)) expect([s, fx]).toEqual([before, []])
      }
      for (const f of fx) {
        // The budget: no read repeats before its period.
        const r = READ_OF(f)
        if (r) expect(t - (last[r] ?? -Infinity)).toBeGreaterThanOrEqual(EVERY[r])
        if (r) last[r] = t
        // The role confines the effects, whatever is sent.
        if (role === 'viewer') expect(['tracks', 'packets', 'topic', 'fit', 'wake', 'screen']).toContain(f.tag)
        // The screens follow the state: the shape is the state's.
        if (f.tag === 'fit') expect(f.ratio).toBe(ratio(s))
        if (role === 'admin' && f.tag === 'fetch') expect(OWNER_ONLY).not.toContain(f.of)
        // One write at a time: a new one starts only from none, or as the second call of `present`.
        if (f.tag === 'fetch' && !r) {
          const w = (before as Following).write
          expect(w === undefined || (w.of === 'present' && f.of === 'slides')).toBe(true)
          expect(s.write).toMatchObject({ seq: f.seq, of: f.of, until: t + WRITE })
        }
      }
      // No key without an expiry ahead of the clock; the last day is `expiring`.
      expect(s.key.tag === 'hidden').toBe(role !== 'owner')
      if ('key' in s.key) expect(s.key.key.endsAt).toBeGreaterThan(t)
      if ('key' in s.key) expect(s.key.tag === 'expiring').toBe(t >= s.key.key.endsAt - SOON)
      // No live stream without a source; no slide state without the bot.
      if (s.stream.tag === 'live') expect(typeof s.stream.source.identity).toBe('string')
      expect(s.topic).toBe(s.show.tag !== 'none')
      // Every error says if and when to retry, never later than the cap.
      const retry = s.error?.retry
      if (retry?.tag === 'at') expect(retry.ms - t).toBeLessThanOrEqual(CAP)
      if (retry?.tag === 'at') expect(retry.ms).toBeGreaterThanOrEqual(0)
      if (s.error) expect(['at', 'never']).toContain(s.error.retry.tag)
    }
    return { s, now, wake }
  }

  const cases = ROLES.flatMap((role) => [1, 2, 3, 4, 5, 6, 7, 8].map((seed) => [role, seed] as const))
  it.each(cases)('hold for a %s on trace %i, and the reads go on when every answer is lost', (role, seed) => {
    const { wake, ...end } = trace(seed, role, 600)
    let { s, now } = end
    if (s.tag !== 'following') return
    // Driven by its wake alone and answered by nobody, it reads the tracks once a second.
    const from = (now = Math.max(now, s.now, wake))
    let reads = 0
    while (now < from + 10 * EVERY.tracks) {
      const [n, fx] = step(s, { tag: 'tick', now })
      expect(n).not.toBe(s)
      reads += fx.filter((f) => f.tag === 'tracks').length
      s = n as Following
      now = Math.max(
        now + 1,
        fx.reduce((at, f) => (f.tag === 'wake' ? f.at : at), 0)
      )
    }
    expect(reads).toBeGreaterThanOrEqual(9)
    expect(reads).toBeLessThanOrEqual(11)
  })

  it.each(ROLES)('a fresh answer decides the state of a %s, whatever came before', (role) => {
    for (let seed = 20; seed < 30; seed++) {
      const { s } = trace(seed, role, 200)
      if (s.tag !== 'following') continue
      expect(tracks(s, CAM).stream).toEqual({ tag: 'live', source: { identity: CAM.identity, kind: 'cast' } })
      if (role !== 'owner') continue
      const end = Math.max(s.now, s.sync.key.at) + 3 * SOON
      expect(poll(s, 'key', 200, key('k9', end)).key).toMatchObject({ key: { key: 'k9', endsAt: end } })
    }
  })
})
