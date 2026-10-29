// DOMAIN: the engine page's crash watchdog (deploy/web/engine/boot.js) and the benchmark stream: a
// crash it confirms, like a panic, ends the stream as a failure instead of leaving the run to the
// harness's timeout, and no text of the engine's reaches the stream unless the engine scrubbed it.
// boot.js is imported as the page does, over a stand-in for the engine module, with the stream the
// host asked for (`window.__bevyPerfStream`). Its watchdog ticks on fake timers.

import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'
// boot.js reads window.__serviceUrl as it is evaluated: the host defines it there
import '../lib/baseDomain'
// @ts-expect-error TS7016: plain JS served as is by the engine page, so it has no declarations
import { createPerfStream, crashBlock } from '../../../deploy/web/engine/perf_stream.js'

const engine = vi.hoisted(() => ({
  /** what the engine still held when it was asked, as the wasm export answers */
  perfTake: vi.fn(() => ({ csv: '', log: '' }))
}))

vi.mock('../../../deploy/web/engine/engine.js', () => ({
  initEngine: () => Promise.resolve(),
  // the engine never gets further than its boot: only the watchdog and the console hooks are under test
  prepareRender: () => new Promise(() => undefined),
  start: vi.fn(),
  applyOptionsToUrlParams: vi.fn(),
  engine_home_scene: vi.fn(),
  engine_terms_accepted: vi.fn(),
  engine_crash_event: vi.fn(() => null),
  engine_set_pulse_certificate_hash: vi.fn(),
  engine_set_perf_stream: vi.fn(),
  engine_perf_take: () => engine.perfTake()
}))

type PageStream = { crash(source: string): unknown; panicked?(block: string): unknown }
type PageWindow = Window & {
  __bevyPerfStream?: { path: string; csv: boolean }
  __perfStream?: PageStream
  __perfAppend?: (name: string, text: string) => void
  __perfPanic?: (block: string) => void
  __engineExit?: () => void
  __engineExited?: boolean
  __engineHeartbeat?: () => void
  reportEngineError?: (err: unknown, source?: string) => void
  __rearmCrashWatchdog?: () => void
  __onEngineCrash?: (message: string, source: string) => void
}
const page = window as PageWindow

/** A panic's block as the engine's panic hook hands it over, scrubbed by the engine. */
const ENGINE_PANIC_BLOCK =
  '13:14:15.678 - <color=#FF5C5C>[panic]</color>: \n' +
  'PanicException: panicked at src/lib.rs:5:9:\n' +
  '    connect wss://h.example/r?access_token=REDACTED\n' +
  'DebugLogReportHandler:LogExceptionInternal\n'

/** The console line `console_error_panic_hook` prints for the same panic: the engine's raw text. */
const PANIC_LINE = 'panicked at src/lib.rs:5:9:\nconnect wss://h.example/r?access_token=PAGESECRET\n\nStack:\n\nError'

const NOW = new Date(Date.UTC(2026, 9, 26, 13, 14, 15, 678))

const realError = console.error
const sent: Array<{ url: string; body: string }> = []
let realStream: PageStream | undefined

/** One watchdog tick (WATCHDOG_MS). */
const tick = () => vi.advanceTimersByTime(2000)

/** Ticks until `done`, at most `limit` times; how many it took. */
function ticksUntil(done: () => boolean, limit = 30): number {
  let ticks = 0
  while (!done() && ticks < limit) {
    tick()
    ticks++
  }
  return ticks
}

/** A stream of the page's own sender to a server that takes everything, for what boot.js does to one. */
function ownStream(take?: () => unknown) {
  const posted: Array<{ url: string; body: string }> = []
  const stream = createPerfStream(
    { path: '/__perf/own', csv: true },
    {
      origin: 'http://localhost',
      wait: () => Promise.resolve(),
      now: () => NOW,
      take,
      fetch: async (url: string, init: { body: Uint8Array | string }) => {
        posted.push({ url: new URL(url).pathname, body: typeof init.body === 'string' ? init.body : new TextDecoder().decode(init.body) })
        return { status: 204 }
      }
    }
  ) as PageStream
  const body = (name: string) =>
    posted
      .filter((r) => r.url === `/__perf/own/${name}`)
      .map((r) => r.body)
      .join('')
  const ended = () => posted.some((r) => r.url === '/__perf/own/end')
  return { stream, body, ended }
}

beforeAll(async () => {
  // before the import: the watchdog's interval starts as boot.js is evaluated
  vi.useFakeTimers()
  page.__bevyPerfStream = { path: '/__perf/run', csv: true }
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init: { body: Uint8Array | string }) => {
      sent.push({ url: new URL(url).pathname, body: typeof init.body === 'string' ? init.body : new TextDecoder().decode(init.body) })
      return { status: 204 }
    })
  )
  // before the import: boot.js wraps console.error, and keeps what it found as the real one
  console.error = vi.fn()
  // @ts-expect-error TS7016: plain JS served as is by the engine page, so it has no declarations
  await import('../../../deploy/web/engine/boot.js')
  await vi.waitFor(() => expect(page.__perfStream).toBeDefined())
  realStream = page.__perfStream
})

afterAll(() => {
  console.error = realError
  vi.unstubAllGlobals()
  vi.useRealTimers()
  delete page.__bevyPerfStream
  delete page.__perfStream
})

beforeEach(() => {
  sent.length = 0
  engine.perfTake.mockClear()
  delete page.__engineExited
  delete page.__onEngineCrash
  page.__rearmCrashWatchdog?.()
})

describe('a crash the watchdog confirms', () => {
  it('ends the benchmark stream as a failure, naming what stopped the engine, as a lost GPU device does', () => {
    const crash = vi.fn()
    const host = vi.fn()
    page.__perfStream = { crash }
    page.__onEngineCrash = host
    console.error('[gpu fatal] the WebGPU device was lost')
    // a reason, never the error's text: only the engine can scrub that
    expect(crash).toHaveBeenCalledExactlyOnceWith('gpu')
    // the host still hears of it, as before
    expect(host).toHaveBeenCalledWith('[gpu fatal] the WebGPU device was lost', 'gpu')
  })

  it('ends it once, however many errors follow', () => {
    const crash = vi.fn()
    page.__perfStream = { crash }
    console.error('[gpu fatal] the WebGPU device was lost')
    console.error('[gpu fatal] the WebGPU device was lost again')
    expect(crash).toHaveBeenCalledTimes(1)
  })

  it('leaves a run that ended itself alone: its frames stopping is no crash', () => {
    const crash = vi.fn()
    page.__perfStream = { crash }
    page.__engineExited = true
    console.error('[gpu fatal] the WebGPU device was lost')
    expect(crash).not.toHaveBeenCalled()
  })

  it('does nothing to a page with no benchmark stream', () => {
    delete page.__perfStream
    const host = vi.fn()
    page.__onEngineCrash = host
    console.error('[gpu fatal] the WebGPU device was lost')
    expect(host).toHaveBeenCalledTimes(1)
  })

  it('still reaches the host when the stream cannot take it', () => {
    page.__perfStream = {
      crash: () => {
        throw new Error('stream gone')
      }
    }
    const host = vi.fn()
    page.__onEngineCrash = host
    console.error('[gpu fatal] the WebGPU device was lost')
    expect(host).toHaveBeenCalledTimes(1)
  })
})

describe('an engine panic', () => {
  it('whose block the engine handed over gets no second one from the console line that follows', async () => {
    const own = ownStream(engine.perfTake)
    page.__perfStream = own.stream
    // the engine's panic hook: the panic's block (what __perfPanic does with it)
    own.stream.panicked?.(ENGINE_PANIC_BLOCK)
    // then `console_error_panic_hook`'s line, with the engine's raw text
    console.error(PANIC_LINE)
    await vi.waitFor(() => expect(own.ended()).toBe(true))
    expect(own.body('log')).toBe(ENGINE_PANIC_BLOCK)
    expect(JSON.parse(own.body('end'))).toMatchObject({ exitCode: 101 })
    expect(engine.perfTake).not.toHaveBeenCalled()
  })

  it("whose block did not arrive ends the run with the page's own block, which holds none of the panic's text", async () => {
    engine.perfTake.mockReturnValueOnce({ csv: '', log: 'held line\n' })
    const own = ownStream(engine.perfTake)
    page.__perfStream = own.stream
    console.error(PANIC_LINE)
    await vi.waitFor(() => expect(own.ended()).toBe(true))
    const log = own.body('log')
    expect(log).toBe('held line\n' + crashBlock('panic', NOW))
    for (const raw of ['PAGESECRET', 'access_token', 'wss://', 'panicked at', 'src/lib.rs', 'Stack']) expect(log).not.toContain(raw)
    expect(JSON.parse(own.body('end'))).toMatchObject({ exitCode: 101 })
  })

  it('is the crash the host hears of even when the stream throws', () => {
    page.__perfStream = {
      crash: () => {
        throw new Error('stream gone')
      }
    }
    const host = vi.fn()
    page.__onEngineCrash = host
    page.__engineHeartbeat?.()
    console.error('panicked at src/lib.rs:5:9:\nseeded panic')
    // the heartbeat stops: the panic corroborates the stall at once, rather than after a hang
    expect(ticksUntil(() => host.mock.calls.length > 0)).toBeLessThan(8)
    expect(host).toHaveBeenCalledExactlyOnceWith('panicked at src/lib.rs:5:9:\nseeded panic', 'panic')
  })
})

describe('a stall', () => {
  it('that an error corroborates is shown at once, and ends the benchmark run only once it lasts as long as a hang', () => {
    const crash = vi.fn()
    const host = vi.fn()
    page.__perfStream = { crash }
    page.__onEngineCrash = host
    page.__engineHeartbeat?.()
    page.reportEngineError?.('a HUD request was rejected', 'unhandledrejection')
    const shown = ticksUntil(() => host.mock.calls.length > 0)
    expect(host).toHaveBeenCalledExactlyOnceWith('a HUD request was rejected', 'unhandledrejection')
    expect(crash).not.toHaveBeenCalled()
    const ended = shown + ticksUntil(() => crash.mock.calls.length > 0)
    // one tick sees the last frame, then eight go by without one (HANG_TICKS)
    expect(ended).toBe(9)
    expect(crash).toHaveBeenCalledExactlyOnceWith('watchdog')
  })

  it('that recovers leaves the benchmark run going', () => {
    const crash = vi.fn()
    const host = vi.fn()
    page.__perfStream = { crash }
    page.__onEngineCrash = host
    page.__engineHeartbeat?.()
    page.reportEngineError?.('a HUD request was rejected', 'unhandledrejection')
    ticksUntil(() => host.mock.calls.length > 0)
    expect(host).toHaveBeenCalledTimes(1)
    // a slow arrival: the frames come back and keep coming
    for (let i = 0; i < 20; i++) {
      page.__engineHeartbeat?.()
      tick()
    }
    expect(crash).not.toHaveBeenCalled()
    expect(host).toHaveBeenCalledTimes(1)
  })

  it('with no error ends the run when it is shown: a hang still fails', () => {
    const crash = vi.fn()
    const host = vi.fn()
    page.__perfStream = { crash }
    page.__onEngineCrash = host
    page.__engineHeartbeat?.()
    expect(ticksUntil(() => host.mock.calls.length > 0)).toBe(9)
    expect(host).toHaveBeenCalledWith(expect.stringContaining('The engine stopped responding'), 'watchdog')
    expect(crash).toHaveBeenCalledExactlyOnceWith('watchdog')
  })
})

describe("the engine's own exit", () => {
  it('stands the watchdog down without a benchmark stream: a manual autopilot run is no crash', () => {
    delete page.__perfStream
    const host = vi.fn()
    page.__onEngineCrash = host
    page.__engineHeartbeat?.()
    page.__engineExit?.()
    expect(page.__engineExited).toBe(true)
    for (let i = 0; i < 20; i++) tick()
    expect(host).not.toHaveBeenCalled()
  })
})

// the stream boot.js made from `window.__bevyPerfStream`; it ends once, so the end is last
describe('the stream the page set up', () => {
  it('takes the panic block the engine hands over', () => {
    const stream = realStream as Required<PageStream>
    const panicked = vi.spyOn(stream, 'panicked').mockImplementation(() => undefined)
    try {
      page.__perfPanic?.(ENGINE_PANIC_BLOCK)
      expect(panicked).toHaveBeenCalledExactlyOnceWith(ENGINE_PANIC_BLOCK)
    } finally {
      panicked.mockRestore()
    }
  })

  it('takes what the engine still held, then ends with the crash in words of its own, over the wire', async () => {
    engine.perfTake.mockReturnValue({ csv: 'held rows\r\n', log: 'held line\n' })
    page.__perfStream = realStream
    page.__perfAppend?.('log', 'handed over\n')
    page.__perfAppend?.('csv', 'sent rows\r\n')
    console.error('[gpu fatal] the WebGPU device was lost: wss://h.example/r?access_token=GPUSECRET')
    await vi.waitFor(() => expect(sent.some((r) => r.url === '/__perf/run/end')).toBe(true))

    expect(engine.perfTake).toHaveBeenCalledTimes(1)
    const body = (path: string) => sent.filter((r) => r.url === path).map((r) => r.body).join('')
    expect(body('/__perf/run/csv')).toBe('sent rows\r\nheld rows\r\n')
    const log = body('/__perf/run/log')
    expect(log.startsWith('handed over\nheld line\n')).toBe(true)
    expect(log).toContain('PanicException: the GPU device was lost\nDebugLogReportHandler:LogExceptionInternal\n')
    expect(log).not.toContain('GPUSECRET')
    expect(log).not.toContain('[gpu fatal]')
    expect(JSON.parse(body('/__perf/run/end'))).toEqual({
      exitCode: 101,
      csvBytes: 'sent rows\r\nheld rows\r\n'.length,
      logBytes: log.length,
      failed: false
    })
  })
})
