import { describe, it, afterEach } from 'node:test'
import assert from 'node:assert/strict'
import { chmod, mkdtemp, readFile, rm, writeFile, stat } from 'node:fs/promises'
import path from 'node:path'
import os from 'node:os'
import { EventEmitter } from 'node:events'
import { browserArgs } from '../scripts/browser-args.mjs'
import { findBrowser, openSession } from '../scripts/capture-browser.mjs'
import { cleanupCapture, installCaptureSignals } from '../scripts/capture-teardown.mjs'

const folders: string[] = []
afterEach(async () => {
  for (const folder of folders.splice(0)) await rm(folder, { recursive: true, force: true })
})
async function script(body: string) {
  const folder = await mkdtemp(path.join(os.tmpdir(), 'capture-browser-test-'))
  folders.push(folder)
  const file = path.join(folder, 'browser')
  await writeFile(file, `#!/bin/sh\nexec '${process.execPath}' -e '${body}' -- "$@"\n`)
  await chmod(file, 0o755)
  return { folder, file }
}

describe('when starting a capture browser', () => {
  it('should skip a non-executable candidate', async () => {
    const { file } = await script('process.exit(0)')
    await chmod(file, 0o644)
    assert.equal(await findBrowser([file, process.execPath]), process.execPath)
  })
  it('should fail promptly if the child exits before publishing its DevTools port', async () => {
    const { file } = await script('process.exit(7)')
    const begin = Date.now()
    await assert.rejects(openSession(file), /exited during startup/)
    assert.ok(Date.now() - begin < 2000)
  })
  it('should cancel startup and remove the browser profile', async () => {
    const { folder, file } = await script('process.exit(0)')
    const marker = path.join(folder, 'profile')
    await writeFile(
      file,
      `#!/bin/sh\nexec '${process.execPath}' -e 'require("node:fs").writeFileSync(${JSON.stringify(marker)}, process.argv.find(x=>x.startsWith("--user-data-dir=")).slice(16)); setInterval(()=>{},1000)' -- "$@"\n`
    )
    const controller = new AbortController()
    const pending = openSession(file, { signal: controller.signal })
    let profile = ''
    for (let i = 0; i < 100; i++) {
      try {
        profile = await readFile(marker, 'utf8')
        break
      } catch {
        await new Promise((resolve) => setTimeout(resolve, 10))
      }
    }
    assert.ok(profile)
    controller.abort(new Error('cancelled startup'))
    await assert.rejects(pending, /cancelled startup/)
    await assert.rejects(stat(profile), { code: 'ENOENT' })
  })
})
describe('when capture cleanup also fails', () => {
  it('should preserve the primary failure and report the cleanup failure', async () => {
    const primary = new Error('capture failed')
    const cleanup = new Error('profile busy')
    const diagnostics: unknown[][] = []
    await cleanupCapture(
      primary,
      async () => {
        throw cleanup
      },
      (...items: unknown[]) => diagnostics.push(items)
    )
    assert.deepEqual(diagnostics, [['Capture cleanup failed:', cleanup]])
  })
  it('should surface cleanup failures after a successful capture', async () => {
    await assert.rejects(
      cleanupCapture(undefined, async () => {
        throw new Error('profile busy')
      }),
      /profile busy/
    )
  })
})
describe('when a capture receives a termination signal', () => {
  for (const [name, code] of [
    ['SIGINT', 130],
    ['SIGTERM', 143]
  ] as const)
    it(`should abort on ${name}, set its exit code and remove handlers`, () => {
      const target = Object.assign(new EventEmitter(), { exitCode: 0 })
      const controller = new AbortController()
      const dispose = installCaptureSignals(controller, target)
      target.emit(name)
      assert.equal(controller.signal.aborted, true)
      assert.equal(target.exitCode, code)
      dispose()
      assert.equal(target.listenerCount('SIGINT') + target.listenerCount('SIGTERM'), 0)
    })
})

// Ported from figma-design-system f63bb11 (the launch flag only).
describe('when a capture launches its browser', () => {
  it('should turn off Brave canvas farbling, so a screenshot always measures the same', () => {
    assert.ok(browserArgs('/tmp/profile').includes('--disable-features=BraveFarbling'))
  })
})

// A DevTools endpoint stand-in: the module reads fetch and WebSocket from globalThis when it loads, so a fresh
// copy is imported while both are stubbed. Every CDP command gets an empty result at once.
class FakeSocket extends EventTarget {
  static readonly OPEN = 1
  static readonly CLOSED = 3
  static last: FakeSocket | undefined
  readyState = 0
  sent: string[] = []
  constructor() {
    super()
    FakeSocket.last = this
    queueMicrotask(() => {
      this.readyState = FakeSocket.OPEN
      this.dispatchEvent(new Event('open'))
    })
  }
  send(data: string) {
    this.sent.push(data)
    const { id } = JSON.parse(data) as { id: number }
    queueMicrotask(() =>
      this.dispatchEvent(
        Object.assign(new Event('message'), { data: JSON.stringify({ id, result: {} }) })
      )
    )
  }
  close() {
    this.readyState = FakeSocket.CLOSED
  }
}
async function stubbedOpenSession(): Promise<typeof openSession> {
  const { fetch, WebSocket } = globalThis
  Object.assign(globalThis, {
    fetch: async () =>
      new Response(JSON.stringify({ webSocketDebuggerUrl: 'ws://127.0.0.1:1/devtools' })),
    WebSocket: FakeSocket
  })
  try {
    const specifier = new URL('../scripts/capture-browser.mjs?stubbed-cdp', import.meta.url).href
    return ((await import(specifier)) as { openSession: typeof openSession }).openSession
  } finally {
    Object.assign(globalThis, { fetch, WebSocket })
  }
}
// A browser that publishes a DevTools port in its profile and then stays up until it is stopped.
const PORT_BROWSER =
  'const dir = process.argv.find(x=>x.startsWith("--user-data-dir=")).slice(16); require("node:fs").writeFileSync(dir + "/DevToolsActivePort", "1\\n"); setInterval(()=>{},1000)'

describe('when a command is sent on a capture session that can no longer answer', () => {
  it('should reject a send after close at once instead of waiting out the CDP timeout', async () => {
    const open = await stubbedOpenSession()
    const { file } = await script(PORT_BROWSER)
    const session = await open(file)
    await session.close()
    const sentBefore = FakeSocket.last!.sent.length
    const begin = Date.now()
    await assert.rejects(session.send('Runtime.evaluate'), /Capture browser session closed/)
    assert.ok(Date.now() - begin < 1000)
    assert.equal(FakeSocket.last!.sent.length, sentBefore)
  })
  it('should reject a send on a socket the browser already closed, before the session is closed', async () => {
    const open = await stubbedOpenSession()
    const { file } = await script(PORT_BROWSER)
    const session = await open(file)
    try {
      FakeSocket.last!.readyState = FakeSocket.CLOSED
      const begin = Date.now()
      await assert.rejects(session.send('Runtime.evaluate'), /Capture browser session closed/)
      assert.ok(Date.now() - begin < 1000)
    } finally {
      await session.close()
    }
  })
})
