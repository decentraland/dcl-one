// @vitest-environment node
import { EventEmitter } from 'node:events'
import { beforeEach, describe, it, expect, vi } from 'vitest'
const io = vi.hoisted(() => ({
  build: vi.fn(),
  mkdtemp: vi.fn(),
  readFile: vi.fn(),
  rm: vi.fn(),
  createServer: vi.fn(),
  findBrowser: vi.fn(),
  openSession: vi.fn()
}))
vi.mock('vite', () => ({ build: io.build }))
vi.mock('node:fs/promises', () => ({ mkdtemp: io.mkdtemp, readFile: io.readFile, rm: io.rm }))
vi.mock('node:http', () => ({ createServer: io.createServer }))
vi.mock('../figma/review/scripts/capture-browser.mjs', () => ({
  findBrowser: io.findBrowser,
  openSession: io.openSession
}))
import { productionBrowser } from './production-browser.helpers'
class FakeServer extends EventEmitter {
  listening = false
  close = vi.fn((done?: (error?: Error) => void) => {
    this.listening = false
    this.emit('close')
    done?.()
  })
  address = () => ({ port: 43210 })
  listen = vi.fn((...args: unknown[]) => {
    this.listening = true
    queueMicrotask(args.at(-1) as () => void)
    return this
  })
}
const deferred = <T>() => {
  let resolve!: (value: T) => void
  let reject!: (error: Error) => void
  const promise = new Promise<T>((a, b) => {
    resolve = a
    reject = b
  })
  return { promise, resolve, reject }
}
let server: FakeServer
let session: { close: ReturnType<typeof vi.fn>; send: ReturnType<typeof vi.fn> }
function fixture() {
  let start!: () => Promise<void>
  let stop!: () => Promise<void>
  const browser = productionBrowser('test/consumer/fixture.tsx', {
    beforeAll: (fn) => {
      start = fn
    },
    afterAll: (fn) => {
      stop = fn
    }
  })
  return { browser, start: () => start(), stop: () => stop() }
}
beforeEach(() => {
  vi.resetAllMocks()
  server = new FakeServer()
  session = { close: vi.fn().mockResolvedValue(undefined), send: vi.fn().mockResolvedValue({}) }
  io.build.mockResolvedValue(undefined)
  io.mkdtemp.mockResolvedValue('/tmp/native-browser-fixture')
  io.rm.mockResolvedValue(undefined)
  io.createServer.mockReturnValue(server)
  io.findBrowser.mockResolvedValue('fixture-browser')
  io.openSession.mockResolvedValue(session)
})
describe('when a production browser fixture owns its complete setup lifetime', () => {
  it('should abort after pending scratch allocation without starting a disposed build', async () => {
    const gate = deferred<string>()
    io.mkdtemp.mockReturnValue(gate.promise)
    const f = fixture()
    const setup = f.start().catch((error) => error)
    await vi.waitFor(() => expect(io.mkdtemp).toHaveBeenCalledOnce())
    const cleanup = f.stop()
    gate.resolve('/tmp/late-native-browser-fixture')
    expect(await setup).toBeInstanceOf(Error)
    await cleanup
    expect(io.build).not.toHaveBeenCalled()
    expect(io.createServer).not.toHaveBeenCalled()
    expect(io.rm).toHaveBeenCalledExactlyOnceWith('/tmp/late-native-browser-fixture', {
      recursive: true,
      force: true
    })
  })

  it('should register lazily and release an unopened fixture without allocating resources', async () => {
    const f = fixture()
    expect(io.build).not.toHaveBeenCalled()
    await f.stop()
    expect(io.mkdtemp).not.toHaveBeenCalled()
    expect(io.createServer).not.toHaveBeenCalled()
    expect(io.openSession).not.toHaveBeenCalled()
  })
  it('should close a completed fixture once and preserve the existing session/origin API', async () => {
    const f = fixture()
    await f.start()
    expect(f.browser.session).toBe(session)
    expect(f.browser.origin).toBe('http://127.0.0.1:43210/')
    const a = f.stop()
    expect(f.stop()).toBe(a)
    await a
    expect(session.close).toHaveBeenCalledOnce()
    expect(server.close).toHaveBeenCalledOnce()
    expect(io.rm).toHaveBeenCalledExactlyOnceWith('/tmp/native-browser-fixture', {
      recursive: true,
      force: true
    })
    expect(server.listenerCount('error')).toBe(0)
  })
  it('should release no nonexistent scratch after allocation failure', async () => {
    const error = new Error('scratch failed')
    io.mkdtemp.mockRejectedValue(error)
    const f = fixture()
    await expect(f.start()).rejects.toBe(error)
    await f.stop()
    expect(io.rm).not.toHaveBeenCalled()
    expect(io.createServer).not.toHaveBeenCalled()
  })
  it('should preserve the build failure and remove only its allocated scratch', async () => {
    const error = new Error('build failed')
    io.build.mockRejectedValue(error)
    const f = fixture()
    await expect(f.start()).rejects.toBe(error)
    await f.stop()
    expect(io.createServer).not.toHaveBeenCalled()
    expect(io.rm).toHaveBeenCalledOnce()
  })
  it('should await an interrupted build and prevent a late server allocation', async () => {
    const gate = deferred<void>()
    io.build.mockReturnValue(gate.promise)
    const f = fixture()
    const setup = f.start().catch((e) => e)
    await vi.waitFor(() => expect(io.build).toHaveBeenCalled())
    const cleanup = f.stop()
    gate.resolve()
    expect(await setup).toBeInstanceOf(Error)
    await cleanup
    expect(io.createServer).not.toHaveBeenCalled()
    expect(io.rm).toHaveBeenCalledOnce()
  })
  it('should preserve listen errors without launching a browser or retaining its error listener', async () => {
    const error = new Error('listen failed')
    server.listen.mockImplementation(() => {
      queueMicrotask(() => server.emit('error', error))
      return server
    })
    const f = fixture()
    await expect(f.start()).rejects.toBe(error)
    await f.stop()
    expect(io.findBrowser).not.toHaveBeenCalled()
    expect(server.listenerCount('error')).toBe(0)
    expect(io.rm).toHaveBeenCalledOnce()
  })
  it('should cancel pending listening through the actual owned AbortSignal', async () => {
    server.listen.mockImplementation((options) => {
      ;(options as { signal: AbortSignal }).signal.addEventListener(
        'abort',
        () => {
          server.close()
        },
        { once: true }
      )
      return server
    })
    const f = fixture()
    const setup = f.start().catch((e) => e)
    await vi.waitFor(() => expect(server.listen).toHaveBeenCalled())
    await f.stop()
    expect(await setup).toBeInstanceOf(Error)
    expect(server.close).toHaveBeenCalledOnce()
    expect(io.openSession).not.toHaveBeenCalled()
    expect(server.listenerCount('error')).toBe(0)
  })
  it('should close the listening server after browser selection fails', async () => {
    const error = new Error('browser missing')
    io.findBrowser.mockRejectedValue(error)
    const f = fixture()
    await expect(f.start()).rejects.toBe(error)
    await f.stop()
    expect(server.close).toHaveBeenCalledOnce()
    expect(io.openSession).not.toHaveBeenCalled()
    expect(io.rm).toHaveBeenCalledOnce()
  })
  it('should not start a late browser after cancellation during selection', async () => {
    const gate = deferred<string>()
    io.findBrowser.mockReturnValue(gate.promise)
    const f = fixture()
    const setup = f.start().catch((e) => e)
    await vi.waitFor(() => expect(io.findBrowser).toHaveBeenCalled())
    const cleanup = f.stop()
    gate.resolve('late-browser')
    await cleanup
    expect(await setup).toBeInstanceOf(Error)
    expect(io.openSession).not.toHaveBeenCalled()
    expect(server.close).toHaveBeenCalledOnce()
  })
  it('should preserve a startup failure and still close its server and scratch', async () => {
    const error = new Error('startup failed')
    io.openSession.mockRejectedValue(error)
    const f = fixture()
    await expect(f.start()).rejects.toBe(error)
    await f.stop()
    expect(server.close).toHaveBeenCalledOnce()
    expect(io.rm).toHaveBeenCalledOnce()
  })
  it('should abort pending startup and await its rejection before releasing resources', async () => {
    io.openSession.mockImplementation(
      (_exe, options) =>
        new Promise((_resolve, reject) => {
          options.signal.addEventListener('abort', () => reject(options.signal.reason), {
            once: true
          })
        })
    )
    const f = fixture()
    const setup = f.start().catch((e) => e)
    await vi.waitFor(() => expect(io.openSession).toHaveBeenCalled())
    await f.stop()
    expect(await setup).toBeInstanceOf(Error)
    expect(session.close).not.toHaveBeenCalled()
    expect(server.close).toHaveBeenCalledOnce()
    expect(io.rm).toHaveBeenCalledOnce()
  })
  it('should close a browser that resolves after the fixture was already disposed', async () => {
    const gate = deferred<typeof session>()
    io.openSession.mockReturnValue(gate.promise)
    const f = fixture()
    const setup = f.start().catch((e) => e)
    await vi.waitFor(() => expect(io.openSession).toHaveBeenCalled())
    const cleanup = f.stop()
    expect(io.openSession.mock.calls[0]![1].signal.aborted).toBe(true)
    gate.resolve(session)
    await cleanup
    expect(await setup).toBeInstanceOf(Error)
    expect(session.send).not.toHaveBeenCalled()
    expect(session.close).toHaveBeenCalledOnce()
    expect(server.close).toHaveBeenCalledOnce()
    expect(io.rm).toHaveBeenCalledOnce()
  })
  it('should retain the first cleanup error while still releasing later resources', async () => {
    const error = new Error('session close failed')
    session.close.mockRejectedValue(error)
    const f = fixture()
    await f.start()
    await expect(f.stop()).rejects.toBe(error)
    expect(server.close).toHaveBeenCalledOnce()
    expect(io.rm).toHaveBeenCalledOnce()
  })
})
