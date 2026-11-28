import { Command, contain, Effect, Event, init, Packet, Role, State, step, Track } from './machine'

/** Everything the reducer's effects need from an explorer. */
export type Io = {
  now(): number
  frame(fn: () => void): void
  realm(): Promise<{ baseUrl: string; preview: boolean; address: string }>
  tracks(): Promise<readonly Track[]>
  packets(): Promise<readonly Packet[]>
  fetch(method: string, url: string, body?: string): Promise<{ status: number; body: string }>
  topic(on: boolean): void
  publish(data: string): void
  screen(entity: number, src: string): void
  video(entity: number): { stamp: number; state: number } | undefined
  /** The entity's width and height; `to` sets them. */
  box(entity: number, to?: readonly [number, number]): readonly [number, number]
}

/** The role decides what the handle can send: a viewer nothing, an admin Cast commands, the owner key commands too. */
export type Cast<R extends Role> = {
  readonly state: State
  onChange(fn: (state: State) => void): void
  /**
   * Shows the stream on this entity's VideoPlayer; the first one reports the video state. The screen
   * keeps the shape of the picture inside the size it has now; `fit: false` leaves its size alone.
   */
  show(screen: number, fit?: boolean): void
  close(): void
} & (R extends 'viewer' ? unknown : { send(cmd: Command<R>): void })

// The clock and the reads' bookkeeping move every second: a scene is told of everything else.
const QUIET = ['now', 'seq', 'stamp', 'sync']
const shown = (state: State) => JSON.stringify(state, (key, value) => (QUIET.includes(key) ? undefined : value))

export function open<R extends Role>(role: R, io: Io): Cast<R> {
  let [state, effects] = init(role, io.now())
  let last = shown(state)
  let wake = 0
  let src = ''
  let ratio = 0
  const screens: number[] = []
  const boxes = new Map<number, readonly [number, number]>()
  const listeners: ((state: State) => void)[] = []
  // A runtime read that fails is a lost answer: the reducer reads again on its own schedule.
  const lost = () => undefined
  const failed = () => ({ status: 0, body: '' })

  function run(f: Effect): unknown {
    switch (f.tag) {
      case 'realm':
        return io.realm().then((r) => send({ tag: 'realm', ...r }), lost)
      case 'tracks':
        return io.tracks().then((tracks) => send({ tag: 'tracks', seq: f.seq, tracks }), lost)
      case 'packets':
        return io.packets().then((packets) => send({ tag: 'packets', seq: f.seq, packets }), lost)
      case 'fetch':
        return io
          .fetch(f.method, f.url, f.body)
          .catch(failed)
          .then((r) => send({ tag: 'answer', seq: f.seq, of: f.of, status: r.status, body: r.body }))
      case 'topic':
        return io.topic(f.on)
      case 'publish':
        return io.publish(f.data)
      case 'screen':
        src = f.src
        return screens.forEach((screen) => io.screen(screen, src))
      case 'fit':
        ratio = f.ratio
        return boxes.forEach((box, screen) => io.box(screen, contain(box, ratio)))
      case 'wake':
        wake = f.at
    }
  }

  function send(e: Event) {
    const before = state
    ;[state, effects] = step(state, e)
    effects.forEach(run)
    if (state === before || last === (last = shown(state))) return
    listeners.forEach((fn) => fn(state))
  }

  effects.forEach(run)
  io.frame(() => {
    const v = screens.length ? io.video(screens[0]) : undefined
    if (v) send({ tag: 'video', ...v })
    if (io.now() >= wake) send({ tag: 'tick', now: io.now() })
  })
  const cast: Cast<'owner'> = {
    get state() {
      return state
    },
    onChange: (fn) => void listeners.push(fn),
    show(screen, fit = true) {
      screens.push(screen)
      if (src) io.screen(screen, src)
      if (fit) boxes.set(screen, io.box(screen))
      if (fit && ratio) io.box(screen, contain(boxes.get(screen)!, ratio))
    },
    close: () => send({ tag: 'close' }),
    send: (cmd) => send({ tag: 'command', cmd })
  }
  // One object serves every role: the type withholds `send`, and the reducer refuses a command above the role.
  return cast as unknown as Cast<R>
}
