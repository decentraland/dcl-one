/**
 * The scene side of streaming and Cast as one state value and one pure reducer.
 * `step` never performs I/O: signed fetches, runtime reads, topic packets, the
 * VideoPlayer source and the timer are returned as data.
 */

export type Role = 'viewer' | 'admin' | 'owner'
export type Read = 'tracks' | 'packets' | 'key' | 'presenters'
export type Verb = 'generate' | 'reset' | 'revoke' | 'link' | 'promote' | 'demote' | 'present' | 'slides'
export type Fetched = 'key' | 'presenters' | Verb

type AdminCommand =
  | { do: 'promote' | 'demote'; who: string }
  | { do: 'slide'; to: 'next' | 'prev' | number }
  | { do: 'play'; video: number }
  | { do: 'pause' | 'stop' }
type OwnerCommand = AdminCommand | { do: 'generate' | 'reset' | 'revoke' | 'link' } | { do: 'present'; file: string }
export type Command<R extends Role = 'owner'> = R extends 'owner'
  ? OwnerCommand
  : R extends 'admin'
    ? AdminCommand
    : never

export type Retry = { tag: 'never' } | { tag: 'at'; ms: number }
export type Code =
  'preview' | 'forbidden' | 'pending' | 'live' | 'busy' | 'noKey' | 'timeout' | 'rejected' | 'unavailable' | 'invalid'
export type CastError = {
  code: Code
  of: Fetched | Command['do'] | 'realm'
  status: number
  message: string
  retry: Retry
}

export type StreamKey = { key: string; endsAt: number; url?: string; links?: { stream: string; watch: string } }
export type Key =
  | { tag: 'hidden' | 'unknown' | 'none' | 'expired' | 'revoked' }
  | { tag: 'active' | 'expiring' | 'replaced'; key: StreamKey }
export type Kind = 'presentation' | 'screen' | 'cast' | 'rtmp' | 'other'
export type Size = { width: number; height: number }
export type Stream = { tag: 'off' } | { tag: 'live'; source: { identity: string; kind: Kind; size?: Size } }
export type Video = 'loading' | 'playing' | 'stalled' | 'paused' | 'error'
const PLAYBACK = ['idle', 'loading', 'playing', 'paused', 'error'] as const
/** The state of the slide's video, as the bot says it. */
export type Playback = (typeof PLAYBACK)[number]
export type Slides = {
  id: string
  file: string
  slide: number
  count: number
  videos: number
  video: Playback
  size?: Size
}
export type Show = { tag: 'none' | 'detected' } | { tag: 'showing'; slides: Slides }

type Sync = { at: number; tries: number; done: number }
type Write = { seq: number; of: Verb; arg: string; until: number }
export type Following = {
  tag: 'following'
  role: Role
  me: string
  base: string
  now: number
  seq: number
  stream: Stream
  video: Video
  stamp: number
  key: Key
  presenters: readonly string[]
  show: Show
  topic: boolean
  sync: Record<Read, Sync>
  write?: Write
  error?: CastError
}
export type State =
  | { tag: 'connecting'; role: Role; at: number; tries: number }
  | { tag: 'unavailable'; error: CastError }
  | Following
  | { tag: 'closed' }

export type Track = { identity: string; trackSid: string; sourceType: number; width?: number; height?: number }
export type Packet = { sender: string; data: string }
type Answer = { tag: 'answer'; seq: number; of: Fetched; status: number; body: string }
export type Event =
  | { tag: 'tick'; now: number }
  | { tag: 'realm'; baseUrl: string; preview: boolean; address: string }
  | { tag: 'tracks'; seq: number; tracks: readonly Track[] }
  | { tag: 'packets'; seq: number; packets: readonly Packet[] }
  | Answer
  | { tag: 'video'; stamp: number; state: number }
  | { tag: 'command'; cmd: Command }
  | { tag: 'close' }

export type Effect =
  | { tag: 'realm' }
  | { tag: 'tracks' | 'packets'; seq: number }
  | { tag: 'fetch'; seq: number; of: Fetched; method: string; url: string; body?: string }
  | { tag: 'topic'; on: boolean }
  | { tag: 'publish'; data: string }
  | { tag: 'screen'; src: string }
  | { tag: 'fit'; ratio: number }
  | { tag: 'wake'; at: number }

/** The only source both explorers resolve; each re-attaches by itself when the track returns. */
export const SRC = 'livekit-video://current-stream'
/** The budget: no read repeats sooner than this (ms), and at most one write is in flight. */
export const EVERY: Record<Read, number> = { tracks: 1_000, packets: 250, key: 60_000, presenters: 60_000 }
export const BASE = 2_000
export const CAP = 300_000
export const SOON = 86_400_000
export const WRITE = 30_000

const RANK: Record<Role, number> = { viewer: 0, admin: 1, owner: 2 }
// Everything else needs an admin, except the tracks and the bot's packets, which every role reads.
const OWNER: readonly string[] = ['key', 'generate', 'reset', 'revoke', 'link', 'present']
const GATEKEEPER = 'comms-gatekeeper'
// The server mints this identity and stamps it on the bot's tracks and packets; a player's is its wallet.
const BOT = 'presentation-bot:'
const CALLS: Record<Fetched, [method: string, service: string, path: string]> = {
  key: ['GET', GATEKEEPER, '/scene-stream-access'],
  generate: ['POST', GATEKEEPER, '/scene-stream-access'],
  reset: ['PUT', GATEKEEPER, '/scene-stream-access'],
  revoke: ['DELETE', GATEKEEPER, '/scene-stream-access'],
  link: ['GET', GATEKEEPER, '/cast/generate-stream-link'],
  presenters: ['GET', GATEKEEPER, '/cast/presenters'],
  promote: ['PUT', GATEKEEPER, '/cast/presenters/'],
  demote: ['DELETE', GATEKEEPER, '/cast/presenters/'],
  present: ['POST', GATEKEEPER, '/cast/presentation-bot-token'],
  slides: ['POST', 'cast-presenter-service', '/presentations']
}
const READS = Object.keys(EVERY) as Read[]
const KINDS: Kind[] = ['presentation', 'screen', 'cast', 'rtmp', 'other']
// VideoState 0..7: Unity reports only playing, paused and none; Bevy repeats its state each second.
const VIDEO: Video[] = ['loading', 'error', 'loading', 'loading', 'playing', 'stalled', 'stalled', 'paused']
const NEVER: Retry = { tag: 'never' }
const PENDING = /place not found/i

const backoff = (tries: number) => Math.min(BASE * 2 ** tries, CAP)
const may = (s: Following, what: Read | Command['do']) =>
  RANK[s.role] >= (what === 'tracks' || what === 'packets' ? 0 : OWNER.includes(what) ? 2 : 1)
// The bot obeys presenters only: an admin that is one asks it for the slide state.
const asks = (s: Following) => s.topic && may(s, 'presenters') && s.presenters.includes(s.me)
const size = (j: any): Size | undefined =>
  j?.width > 0 && j.height > 0 ? { width: +j.width, height: +j.height } : undefined
const shape = (of?: Size | false) => (of ? of.width / of.height : 0)
const active = (s: Following, r: Read) =>
  may(s, r) && (r === 'packets' ? s.topic : r === 'presenters' ? s.stream.tag === 'live' : true)
const ok = (e: Answer) => e.status >= 200 && e.status < 300
const json = (body: string): any => {
  try {
    return JSON.parse(body)
  } catch {
    return undefined
  }
}

/** A Decentraland realm has one host for each service; any other realm serves them under its origin. */
export function serviceUrl(baseUrl: string, service: string, path: string): string {
  const origin = /^https?:\/\/[^/?#]+/i.exec(baseUrl)?.[0]
  const tld = /\/\/(?:[a-z0-9-]+\.)*decentraland\.(org|zone|today)(?::\d+)?$/i.exec(origin ?? '')?.[1]
  return origin && !tld ? `${origin}/${service}${path}` : `https://${service}.decentraland.${tld ?? 'org'}${path}`
}

/** Width over height of the picture on the screens; 0 while neither the bot nor the client says it. */
export const ratio = (s: Following): number =>
  shape((s.show.tag === 'showing' && s.show.slides.size) || (s.stream.tag === 'live' && s.stream.source.size))

/** The largest width and height of that ratio inside a box; the box itself while the ratio is not known. */
export const contain = ([w, h]: readonly [number, number], ratio: number): readonly [number, number] =>
  ratio > 0 ? (w / h > ratio ? [h * ratio, h] : [w, w / ratio]) : [w, h]

function kindOf(t: Track): Kind | undefined {
  if (t.identity.startsWith(BOT)) return 'presentation'
  if (t.sourceType === 2) return 'screen'
  if (t.identity.startsWith('stream:')) return 'cast'
  if (t.identity.endsWith('-streamer')) return 'rtmp'
  return t.trackSid === SRC ? 'other' : undefined
}

// The server accepts a key exactly while now is before its expiry; so does this.
function age(k: Key, now: number): Key {
  if (!('key' in k)) return k
  const end = k.key.endsAt
  const tag = now >= end ? 'expired' : now >= end - SOON ? 'expiring' : k.tag === 'expiring' ? 'active' : k.tag
  return tag === k.tag ? k : tag === 'expired' ? { tag } : { tag, key: k.key }
}

// Both key shapes: /scene-stream-access (snake case, ms) and the Cast link (camel case, ISO time).
function parseKey(j: any): StreamKey | undefined {
  const key = j?.streaming_key ?? j?.streamingKey
  const endsAt = typeof j?.ends_at === 'number' ? j.ends_at : Date.parse(j?.expiresAt)
  if (typeof key !== 'string' || !key || !(endsAt > 0)) return
  const k: StreamKey = { key, endsAt }
  if (typeof j.streaming_url === 'string') k.url = j.streaming_url
  if (typeof j.streamLink === 'string' && typeof j.watcherLink === 'string')
    k.links = { stream: j.streamLink, watch: j.watcherLink }
  return k
}

// The key string is the credential's identity: the same string adds knowledge, another one replaces it.
function hold(s: Following, k: StreamKey, own: boolean): Key {
  const prev = s.key
  const same = 'key' in prev && prev.key.key === k.key
  const tag = same ? prev.tag : own || !('key' in prev) ? 'active' : 'replaced'
  return age({ tag, key: same ? { ...prev.key, ...k } : k }, s.now)
}

function classify(e: Answer): [Code, boolean] {
  const { status } = e
  if (ok(e)) return ['invalid', true]
  if (status === 0 || status === 408 || status === 429 || status >= 500) return ['unavailable', true]
  if (status === 409) return /try again/i.test(e.body) ? ['unavailable', true] : ['live', false]
  if (status === 404 && PENDING.test(e.body)) return ['pending', true]
  return [status === 401 || status === 403 ? 'forbidden' : 'rejected', false]
}

function fail(s: Following, of: CastError['of'], code: Code, retry: Retry, e?: Answer): Following {
  const j = e && json(e.body)
  const message = e ? String(j?.error ?? j?.message ?? e.body).slice(0, 200) : ''
  return { ...s, error: { code, of, status: e?.status ?? 0, message, retry } }
}

const settled = (s: Following, of: Fetched): Following => (s.error?.of === of ? { ...s, error: undefined } : s)

function write(s: Following, of: Verb, arg: string, fx: Effect[], body?: string): Following {
  const seq = s.seq + 1
  fx.push(fetch(s, seq, of, arg, body))
  return { ...s, seq, write: { seq, of, arg, until: s.now + WRITE } }
}

function fetch(s: Following, seq: number, of: Fetched, arg = '', body?: string): Effect {
  const [method, service, path] = CALLS[of]
  const url = serviceUrl(s.base, service, path.endsWith('/') ? path + arg : path)
  return { tag: 'fetch', seq, of, method, url, body }
}

// An answer counts once and only when it is newer than the last one applied:
// a duplicated, reordered or never issued answer changes nothing.
function fresh(s: Following, r: Read, seq: number): Following | undefined {
  const y = s.sync[r]
  if (may(s, r) && seq > y.done && seq <= s.seq) return { ...s, sync: { ...s.sync, [r]: { ...y, done: seq } } }
}

type Patch = Partial<Following> | undefined | false

const keyed = (s: Following, j: any): Patch => {
  const k = parseKey(j)
  return k && { key: hold(s, k, true) }
}

const POLLED: Record<'key' | 'presenters', (s: Following, e: Answer, j: any) => Patch> = {
  key(s, e, j) {
    const k = ok(e) && parseKey(j)
    if (k) return { key: hold(s, k, false) }
    if (e.status !== 404 || PENDING.test(e.body)) return
    if (/expired/i.test(e.body)) return { key: { tag: 'expired' } }
    return { key: 'key' in s.key ? { tag: 'revoked' } : s.key.tag === 'unknown' ? { tag: 'none' } : s.key }
  },
  presenters: (_, e, j) =>
    ok(e) && Array.isArray(j?.presenters)
      ? { presenters: j.presenters.map((p: unknown) => String(p).toLowerCase()) }
      : e.status === 404 && { presenters: [] }
}

const WRITTEN: Record<Verb, (s: Following, j: any, arg: string, fx: Effect[]) => Patch> = {
  generate: keyed,
  reset: keyed,
  link: keyed,
  revoke: () => ({ key: { tag: 'revoked' } }),
  promote: (s, _, who) => ({ presenters: s.presenters.includes(who) ? s.presenters : [...s.presenters, who] }),
  demote: (s, _, who) => ({ presenters: s.presenters.filter((p) => p !== who) }),
  present: (s, j, file, fx) =>
    typeof j?.url === 'string' &&
    typeof j?.token === 'string' &&
    write(s, 'slides', file, fx, JSON.stringify({ url: file, livekitToken: j.token, livekitUrl: j.url })),
  slides: () => ({})
}

const PACKET: Record<'slide' | 'play' | 'pause' | 'stop', (c: any) => object> = {
  slide: ({ to }) => ({
    type: 'presentation:navigate',
    ...(typeof to === 'number' ? { action: 'goto', slideIndex: to } : { action: to })
  }),
  play: (c) => ({ type: 'presentation:video:play', videoIndex: c.video }),
  pause: () => ({ type: 'presentation:video:pause' }),
  stop: () => ({ type: 'presentation:stop' })
}

type Handlers = {
  [T in Event['tag']]?: (s: Following, e: Extract<Event, { tag: T }>, fx: Effect[]) => Following | undefined
}
const FOLLOW: Handlers = {
  tick(s, { now }, fx) {
    if (now < s.now) return
    let n: Following = { ...s, now, key: age(s.key, now), sync: { ...s.sync } }
    // Explorers put no deadline on a signed fetch: a write without an answer ends here.
    if (n.write && now >= n.write.until) n = fail({ ...n, write: undefined }, n.write.of, 'timeout', NEVER)
    for (const r of READS) {
      if (!active(n, r) || now < n.sync[r].at) continue
      n.seq += 1
      n.sync[r] = { ...n.sync[r], at: now + EVERY[r] }
      fx.push(r === 'tracks' || r === 'packets' ? { tag: r, seq: n.seq } : fetch(n, n.seq, r))
    }
    return n
  },

  tracks(s, e) {
    const n = fresh(s, 'tracks', e.seq)
    if (!n) return
    const kinds = e.tracks.map(kindOf)
    const best = KINDS.find((k) => kinds.includes(k))
    const show: Show = best !== 'presentation' ? { tag: 'none' } : s.show.tag === 'none' ? { tag: 'detected' } : s.show
    const at = best && e.tracks[kinds.indexOf(best)]
    const source = at && { identity: at.identity, kind: best, size: size(at) }
    // The presenters are read only while a stream is live: without one the last list says nothing.
    const presenters = source ? n.presenters : []
    return { ...n, show, presenters, stream: source ? { tag: 'live', source } : { tag: 'off' } }
  },

  packets(s, e) {
    const n = fresh(s, 'packets', e.seq)
    if (!n) return
    let show = n.show
    for (const p of e.packets) {
      if (show.tag === 'none') break
      // Any wallet in the room can publish on the topic: only the bot states the slides.
      if (!p.sender.startsWith(BOT)) continue
      const j = json(p.data)
      if (j?.type === 'presentation:stopped') show = { tag: 'detected' }
      if (j?.type !== 'presentation:state' || typeof j.id !== 'string') continue
      const videos = Array.isArray(j.slideVideos) ? j.slideVideos.length : 0
      const slides = { id: j.id, file: String(j.fileName ?? ''), slide: j.currentSlide | 0, count: j.slideCount | 0 }
      const video: Playback = PLAYBACK.includes(j.videoState) ? j.videoState : 'idle'
      show = { tag: 'showing', slides: { ...slides, videos, video, size: size(j.slide) } }
    }
    return { ...n, show }
  },

  video: (s, e) => (e.stamp > s.stamp ? { ...s, stamp: e.stamp, video: VIDEO[e.state] ?? 'loading' } : undefined),

  answer(s, e, fx) {
    const j = json(e.body)
    if (e.of === 'key' || e.of === 'presenters') {
      const n = fresh(s, e.of, e.seq)
      if (!n) return
      const y = n.sync[e.of]
      const got = POLLED[e.of](n, e, j)
      if (!got) {
        // A poll always retries: with backoff when the refusal can pass, at the cap when it is final.
        const [code, again] = classify(e)
        const at = Math.max(y.at, n.now + (again ? backoff(y.tries) : CAP))
        const sync = { ...n.sync, [e.of]: { ...y, at, tries: y.tries + 1 } }
        return fail({ ...n, sync }, e.of, code, { tag: 'at', ms: at }, e)
      }
      const m = settled({ ...n, ...got, sync: { ...n.sync, [e.of]: { ...y, tries: 0 } } }, e.of)
      // The bot sends slide state to presenters only: an admin that is not one yet promotes itself.
      const promote = e.of === 'presenters' && m.show.tag !== 'none' && !m.presenters.includes(m.me) && !m.write
      return promote ? write(m, 'promote', m.me, fx) : m
    }
    const w = s.write
    if (!w || w.seq !== e.seq || w.of !== e.of) return
    const n = { ...s, write: undefined }
    const got = ok(e) && WRITTEN[w.of](n, j, w.arg, fx)
    if (got) return settled({ ...n, ...got }, w.of)
    // A write is never repeated for the caller: the error says when the caller may.
    const [code, again] = classify(e)
    return fail(n, w.of, code, again ? { tag: 'at', ms: n.now + backoff(0) } : NEVER, e)
  },

  command(s, { cmd }, fx) {
    if (!may(s, cmd.do)) return fail(s, cmd.do, 'forbidden', NEVER)
    if (cmd.do === 'slide' || cmd.do === 'play' || cmd.do === 'pause' || cmd.do === 'stop') {
      if (s.show.tag === 'none') return
      fx.push({ tag: 'publish', data: JSON.stringify(PACKET[cmd.do](cmd)) })
      return
    }
    if (s.write) return fail(s, cmd.do, 'busy', { tag: 'at', ms: s.write.until })
    if (cmd.do !== 'present') return write(s, cmd.do, 'who' in cmd ? cmd.who.toLowerCase() : '', fx)
    if (!('key' in s.key)) return fail(s, 'present', 'noKey', NEVER)
    return write(s, 'present', cmd.file, fx, JSON.stringify({ streamingKey: s.key.key.key }))
  }
}

function wakeAt(s: Following): number {
  let at = s.write ? s.write.until : Infinity
  for (const r of READS) if (active(s, r)) at = Math.min(at, s.sync[r].at)
  if ('key' in s.key) at = Math.min(at, s.key.key.endsAt - (s.key.tag === 'expiring' ? 0 : SOON))
  return at
}

export function init(role: Role, now: number): [State, Effect[]] {
  const at = now + backoff(0)
  return [{ tag: 'connecting', role, at, tries: 0 }, [{ tag: 'realm' }, { tag: 'wake', at }]]
}

export function step(s: State, e: Event): [State, Effect[]] {
  if (s.tag === 'closed') return [s, []]
  if (e.tag === 'close')
    return [{ tag: 'closed' }, s.tag === 'following' && s.topic ? [{ tag: 'topic', on: false }] : []]
  if (s.tag === 'unavailable') return [s, []]
  if (s.tag === 'connecting') {
    // A lost realm answer is read again, with backoff.
    if (e.tag === 'tick' && e.now >= s.at) {
      const at = e.now + backoff(s.tries + 1)
      return [{ ...s, at, tries: s.tries + 1 }, [{ tag: 'realm' }, { tag: 'wake', at }]]
    }
    if (e.tag !== 'realm') return [s, []]
    if (e.preview) {
      const error: CastError = { code: 'preview', of: 'realm', status: 0, message: '', retry: NEVER }
      return [{ tag: 'unavailable', error }, []]
    }
    const now = s.at - backoff(s.tries)
    const sync = { at: now, tries: 0, done: 0 }
    const n: Following = {
      tag: 'following',
      role: s.role,
      me: e.address.toLowerCase(),
      base: e.baseUrl,
      now,
      seq: 0,
      stream: { tag: 'off' },
      video: 'loading',
      stamp: -1,
      key: { tag: s.role === 'owner' ? 'unknown' : 'hidden' },
      presenters: [],
      show: { tag: 'none' },
      topic: false,
      sync: { tracks: sync, packets: sync, key: sync, presenters: sync }
    }
    return [
      n,
      [
        { tag: 'screen', src: SRC },
        { tag: 'wake', at: now }
      ]
    ]
  }
  const fx: Effect[] = []
  let n = (FOLLOW[e.tag] as ((s: Following, e: Event, fx: Effect[]) => Following | undefined) | undefined)?.(s, e, fx)
  if (!n) return [s, fx]
  // Every role listens to the bot while its track is there; that sends nothing.
  const topic = n.show.tag !== 'none'
  if (topic !== n.topic) {
    n = { ...n, topic }
    fx.push({ tag: 'topic', on: topic })
  }
  if (asks(n) && !asks(s)) fx.push({ tag: 'publish', data: '{"type":"presentation:get-state"}' })
  if (ratio(n) !== ratio(s)) fx.push({ tag: 'fit', ratio: ratio(n) })
  const at = wakeAt(n)
  if (at !== wakeAt(s)) fx.push({ tag: 'wake', at })
  return [n, fx]
}
