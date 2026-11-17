import { describe, it, afterEach } from 'node:test'
import assert from 'node:assert/strict'
import { spawn, type ChildProcess } from 'node:child_process'
import { createServer, request, type Server } from 'node:http'
import { once } from 'node:events'
import { mkdtemp, stat, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { closeServer, removeProfile, stopBrowser } from '../scripts/capture-teardown.mjs'

// A stand-in browser: a Node child that is ready once it prints, optionally ignoring SIGTERM.
async function browser(ignoreSigterm: boolean): Promise<ChildProcess> {
  const child = spawn(
    process.execPath,
    [
      '-e',
      `${ignoreSigterm ? "process.on('SIGTERM', () => {});" : ''} setInterval(() => {}, 1000); console.log('ready')`
    ],
    { stdio: ['ignore', 'pipe', 'ignore'] }
  )
  await once(child.stdout!, 'data')
  return child
}

// A server with a request still in flight, as a browser loading Review leaves behind. server.close() already
// drops idle keep-alive sockets; only closeAllConnections() ends an active one.
async function serverWithOpenConnection(): Promise<Server> {
  const server = createServer(() => {})
  server.listen(0, '127.0.0.1')
  await once(server, 'listening')
  const { port } = server.address() as { port: number }
  const received = once(server, 'request')
  const pending = request({ port, host: '127.0.0.1' }).on('error', () => {})
  pending.end()
  await received
  // Even when a test fails, nothing may keep the test process alive.
  cleanups.push(() => {
    pending.destroy()
    server.closeAllConnections()
    server.close()
  })
  return server
}

// server.close() only stops listening; 'close' fires once every connection is gone.
const closed = (server: Server) =>
  Promise.race([
    once(server, 'close').then(() => true),
    new Promise<boolean>((resolve) => setTimeout(() => resolve(false), 1000))
  ])

const started: ChildProcess[] = []
const cleanups: (() => void)[] = []
afterEach(() => {
  for (const child of started.splice(0)) if (child.exitCode === null) child.kill('SIGKILL')
  for (const cleanup of cleanups.splice(0)) cleanup()
})

describe('when a Review capture tears down', () => {
  it('should stop a browser that honours SIGTERM without waiting for the grace period', async () => {
    const child = await browser(false)
    started.push(child)
    const server = await serverWithOpenConnection()
    const isClosed = closed(server)
    const begin = Date.now()

    await stopBrowser({ browser: child, graceMs: 5000 })
    closeServer(server)

    assert.equal(child.signalCode, 'SIGTERM')
    assert.ok(Date.now() - begin < 2000)
    assert.equal(await isClosed, true)
  })

  it('should SIGKILL a hung browser that ignores SIGTERM, so the capture error is not hidden', async () => {
    const child = await browser(true)
    started.push(child)

    await stopBrowser({ browser: child, graceMs: 200 })

    assert.equal(child.signalCode, 'SIGKILL')
  })

  it('should return at once when the browser already exited', async () => {
    const child = await browser(false)
    started.push(child)
    child.kill('SIGKILL')
    await once(child, 'exit')
    const server = await serverWithOpenConnection()
    const isClosed = closed(server)
    const begin = Date.now()

    await stopBrowser({ browser: child, graceMs: 5000 })
    closeServer(server)

    assert.ok(Date.now() - begin < 1000)
    assert.equal(await isClosed, true)
  })

  it('should close the DevTools socket before stopping the browser', async () => {
    const child = await browser(false)
    started.push(child)
    const calls: string[] = []

    await stopBrowser({ socket: { close: () => calls.push('socket') }, browser: child })

    assert.deepEqual(calls, ['socket'])
    assert.equal(child.signalCode, 'SIGTERM')
  })
})

describe('when a Review capture removes its browser profile', () => {
  const failing = (code: string, times: number) => {
    let calls = 0
    const remove = async () => {
      calls += 1
      if (calls <= times) throw Object.assign(new Error(code), { code })
    }
    return { remove, calls: () => calls }
  }

  it('should retry while a browser helper still holds the profile, then remove it', async () => {
    const { remove, calls } = failing('ENOTEMPTY', 3)

    await removeProfile('/tmp/profile', { remove, delay: 1 })

    assert.equal(calls(), 4)
  })

  for (const code of ['EBUSY', 'ENOTEMPTY', 'EPERM', 'EMFILE', 'ENFILE'])
    it(`should retry ${code}`, async () => {
      const { remove, calls } = failing(code, 1)

      await removeProfile('/tmp/profile', { remove, delay: 1 })

      assert.equal(calls(), 2)
    })

  it('should fail loudly once the retry budget is spent', async () => {
    const { remove, calls } = failing('EBUSY', Infinity)

    await assert.rejects(removeProfile('/tmp/profile', { remove, retries: 10, delay: 1 }), {
      message: 'Could not remove the capture profile after 11 attempts (EBUSY): /tmp/profile'
    })
    assert.equal(calls(), 11)
  })

  it('should not retry an error that waiting cannot fix', async () => {
    const { remove, calls } = failing('EACCES', Infinity)

    await assert.rejects(removeProfile('/tmp/profile', { remove, delay: 1 }), { code: 'EACCES' })
    assert.equal(calls(), 1)
  })

  it('should remove a real profile directory', async () => {
    const profile = await mkdtemp(path.join(os.tmpdir(), 'review-profile-'))
    await writeFile(path.join(profile, 'Cookies'), 'x')

    await removeProfile(profile)

    await assert.rejects(stat(profile), { code: 'ENOENT' })
  })
})
