import { describe, it, afterEach, beforeEach } from 'node:test'
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
  let child: ChildProcess

  describe('and the browser honours SIGTERM', () => {
    let isClosed: Promise<boolean>
    let elapsed: number

    beforeEach(async () => {
      child = await browser(false)
      started.push(child)
      const server = await serverWithOpenConnection()
      isClosed = closed(server)
      const begin = Date.now()

      await stopBrowser({ browser: child, graceMs: 5000 })
      closeServer(server)
      elapsed = Date.now() - begin
    })

    it('should stop the browser with SIGTERM', () => {
      assert.equal(child.signalCode, 'SIGTERM')
    })

    it('should not wait for the grace period', () => {
      assert.ok(elapsed < 2000)
    })

    it('should close the server with its open connection', async () => {
      assert.equal(await isClosed, true)
    })
  })

  describe('and the browser hangs ignoring SIGTERM', () => {
    beforeEach(async () => {
      child = await browser(true)
      started.push(child)

      await stopBrowser({ browser: child, graceMs: 200 })
    })

    it('should SIGKILL it, so the capture error is not hidden', () => {
      assert.equal(child.signalCode, 'SIGKILL')
    })
  })

  describe('and the browser already exited', () => {
    let isClosed: Promise<boolean>
    let elapsed: number

    beforeEach(async () => {
      child = await browser(false)
      started.push(child)
      child.kill('SIGKILL')
      await once(child, 'exit')
      const server = await serverWithOpenConnection()
      isClosed = closed(server)
      const begin = Date.now()

      await stopBrowser({ browser: child, graceMs: 5000 })
      closeServer(server)
      elapsed = Date.now() - begin
    })

    it('should return at once', () => {
      assert.ok(elapsed < 1000)
    })

    it('should close the server with its open connection', async () => {
      assert.equal(await isClosed, true)
    })
  })

  describe('and a DevTools socket is open', () => {
    let calls: string[]
    let signalAtSocketClose: NodeJS.Signals | null | undefined

    beforeEach(async () => {
      child = await browser(false)
      started.push(child)
      calls = []
      signalAtSocketClose = undefined

      await stopBrowser({
        socket: {
          close: () => {
            calls.push('socket')
            signalAtSocketClose = child.signalCode
          }
        },
        browser: child
      })
    })

    it('should close the DevTools socket', () => {
      assert.deepEqual(calls, ['socket'])
    })

    it('should close the socket while the browser is still running', () => {
      assert.equal(signalAtSocketClose, null)
    })

    it('should stop the browser with SIGTERM', () => {
      assert.equal(child.signalCode, 'SIGTERM')
    })
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

  let remover: ReturnType<typeof failing>

  describe('and a browser helper still holds the profile', () => {
    beforeEach(async () => {
      remover = failing('ENOTEMPTY', 3)

      await removeProfile('/tmp/profile', { remove: remover.remove, delay: 1 })
    })

    it('should retry, then remove it', () => {
      assert.equal(remover.calls(), 4)
    })
  })

  for (const code of ['EBUSY', 'ENOTEMPTY', 'EPERM', 'EMFILE', 'ENFILE'])
    describe(`and the removal fails once with ${code}`, () => {
      beforeEach(async () => {
        remover = failing(code, 1)

        await removeProfile('/tmp/profile', { remove: remover.remove, delay: 1 })
      })

      it(`should retry ${code}`, () => {
        assert.equal(remover.calls(), 2)
      })
    })

  describe('and the retry budget is spent', () => {
    let outcome: Promise<void>

    beforeEach(() => {
      remover = failing('EBUSY', Infinity)
      outcome = removeProfile('/tmp/profile', { remove: remover.remove, retries: 10, delay: 1 })
      // Handled here so the rejection is not reported before the it() awaits it.
      outcome.catch(() => {})
    })

    it('should fail loudly with the attempt count, code and profile', async () => {
      await assert.rejects(outcome, {
        message: 'Could not remove the capture profile after 11 attempts (EBUSY): /tmp/profile'
      })
    })

    it('should stop after the budgeted attempts', async () => {
      await outcome.catch(() => undefined)
      assert.equal(remover.calls(), 11)
    })
  })

  describe('and the error is one that waiting cannot fix', () => {
    let outcome: Promise<void>

    beforeEach(() => {
      remover = failing('EACCES', Infinity)
      outcome = removeProfile('/tmp/profile', { remove: remover.remove, delay: 1 })
      outcome.catch(() => {})
    })

    it('should rethrow the original error', async () => {
      await assert.rejects(outcome, { code: 'EACCES', message: 'EACCES' })
    })

    it('should not retry', async () => {
      await outcome.catch(() => undefined)
      assert.equal(remover.calls(), 1)
    })
  })

  describe('and the profile is a real directory', () => {
    let profile: string

    beforeEach(async () => {
      profile = await mkdtemp(path.join(os.tmpdir(), 'review-profile-'))
      await writeFile(path.join(profile, 'Cookies'), 'x')

      await removeProfile(profile)
    })

    it('should remove it', async () => {
      await assert.rejects(stat(profile), { code: 'ENOENT' })
    })
  })
})
