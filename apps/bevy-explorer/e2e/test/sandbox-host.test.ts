import assert from 'node:assert/strict'
import { test, type TestContext } from 'node:test'

const hostUrl = new URL('../../../../rendering/bevy-explorer/source/deploy/web/engine/sandbox_host.js', import.meta.url)

async function harness(t: TestContext, compile: () => Promise<unknown>) {
  t.mock.timers.enable({ apis: ['setTimeout'] })
  let now = 0
  t.mock.method(performance, 'now', () => now)
  const wasm = Reflect.get(globalThis, 'WebAssembly')
  const compilation = t.mock.method(wasm, 'compileStreaming', compile)
  t.mock.method(globalThis, 'fetch', async () => new Response())
  const originalWorker = Object.getOwnPropertyDescriptor(globalThis, 'Worker')
  const originalErrorEvent = Object.getOwnPropertyDescriptor(globalThis, 'ErrorEvent')
  class Worker {
    onmessage: ((event: unknown) => void) | null = null
    postMessage(message: { payload: { killToken: string } }) {
      queueMicrotask(() => this.onmessage?.({ data: { type: 'INIT_COMPLETE', killToken: message.payload.killToken } }))
    }
  }
  class ErrorEvent {
    constructor(_type: string, fields: object) { Object.assign(this, fields) }
  }
  Object.defineProperty(globalThis, 'Worker', { configurable: true, value: Worker })
  Object.defineProperty(globalThis, 'ErrorEvent', { configurable: true, value: ErrorEvent })
  t.after(() => {
    if (originalWorker) Object.defineProperty(globalThis, 'Worker', originalWorker)
    else Reflect.deleteProperty(globalThis, 'Worker')
    if (originalErrorEvent) Object.defineProperty(globalThis, 'ErrorEvent', originalErrorEvent)
    else Reflect.deleteProperty(globalThis, 'ErrorEvent')
  })
  const { createSandboxHost } = await import(`${hostUrl.href}?test=${t.name}`)
  const crashes = t.mock.fn()
  const host = createSandboxHost({ compiledModule: {}, sharedMemory: {}, onWorkerCrash: crashes })
  const advance = async (ms: number) => {
    now += ms
    t.mock.timers.tick(ms)
    for (let i = 0; i < 8; i++) await Promise.resolve()
  }
  return { host, crashes, compilation, advance, wasm }
}

test('sandbox compilation retries transient failure and starts as soon as compilation succeeds', async (t) => {
  let attempts = 0
  const h = await harness(t, async () => {
    if (++attempts === 1) throw new Error('unavailable')
    return {}
  })
  const started = h.host.spawn()
  await h.advance(0)
  assert.equal(h.compilation.mock.callCount(), 1)
  await h.advance(100)
  await started
  assert.equal(h.compilation.mock.callCount(), 2)
  assert.equal(h.crashes.mock.callCount(), 0)
})

test('sandbox compilation stops retrying invalid WebAssembly immediately', async (t) => {
  const CompileError = Reflect.get(globalThis, 'WebAssembly').CompileError
  const h = await harness(t, async () => { throw new CompileError('invalid') })
  await assert.rejects(h.host.spawn(), /invalid/)
  assert.equal(h.compilation.mock.callCount(), 1)
  assert.equal(h.crashes.mock.callCount(), 1)
})

test('sandbox compilation stops retrying failures at the deadline and a later scene tries again', async (t) => {
  let available = false
  const h = await harness(t, async () => {
    if (!available) throw new Error('unavailable')
    return {}
  })
  const rejected = assert.rejects(h.host.spawn(), /unavailable/)
  for (let step = 0; step < 16; step++) await h.advance(5_000)
  await rejected
  assert.equal(h.crashes.mock.callCount(), 1)
  const failed = h.compilation.mock.callCount()
  assert.ok(failed > 2 && failed < 20, `retried ${failed} times`)
  available = true
  await h.host.spawn()
  assert.equal(h.compilation.mock.callCount(), failed + 1)
})

test('a slow scene runtime download in flight is never cut short', async (t) => {
  let deliver: (module: object) => void = () => {}
  const h = await harness(t, () => new Promise<object>((resolve) => { deliver = resolve }))
  const started = h.host.spawn()
  await h.advance(90_000)
  assert.equal(h.crashes.mock.callCount(), 0)
  assert.equal(h.compilation.mock.callCount(), 1)
  deliver({})
  await started
  assert.equal(h.compilation.mock.callCount(), 1)
})
