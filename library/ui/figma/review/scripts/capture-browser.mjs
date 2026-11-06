import { setTimeout, clearTimeout } from 'node:timers'
const { fetch, WebSocket, AbortSignal } = globalThis
import { spawn } from 'node:child_process'
import { constants } from 'node:fs'
import { access, readFile, mkdtemp } from 'node:fs/promises'
import path from 'node:path'
import os from 'node:os'
import { setTimeout as delay } from 'node:timers/promises'
import { CaptureTimeout } from './capture-retry.mjs'
import { cleanupCapture, removeProfile, stopBrowser } from './capture-teardown.mjs'

async function findBrowser(candidates) {
  for (const candidate of candidates.filter(Boolean)) {
    try {
      await access(candidate, constants.X_OK)
      return candidate
    } catch {
      /* Try the next executable. */
    }
  }
  throw new Error('A Chromium browser is required. Set BROWSER_PATH to Chrome or Chromium.')
}

async function openSession(executable, { signal } = {}) {
  const profile = await mkdtemp(path.join(os.tmpdir(), 'dcl-visual-'))
  const browser = spawn(
    executable,
    [
      '--headless=new',
      '--no-sandbox',
      '--disable-extensions',
      '--disable-background-networking',
      '--force-color-profile=srgb',
      '--remote-debugging-port=0',
      '--user-data-dir=' + profile
    ],
    { stdio: 'ignore' }
  )
  let socket
  let closing
  const pending = new Map()
  let startupError
  browser.on('error', (error) => {
    startupError = error
  })
  const close = () => {
    closing ??= (async () => {
      for (const reject of pending.values()) reject(new Error('Capture browser session closed'))
      pending.clear()
      try {
        await stopBrowser({ socket, browser })
      } finally {
        await removeProfile(profile)
      }
    })()
    return closing
  }
  try {
    let port
    for (let i = 0; i < 100; i++) {
      signal?.throwIfAborted()
      if (startupError) throw startupError
      if (browser.exitCode !== null || browser.signalCode !== null)
        throw new Error('Capture browser exited during startup')
      try {
        port = (await readFile(path.join(profile, 'DevToolsActivePort'), 'utf8')).split('\n')[0]
        break
      } catch {
        await delay(100)
      }
    }
    if (!port) throw new Error('Capture browser did not start')
    const tab = await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, {
      method: 'PUT',
      signal: signal
        ? AbortSignal.any([signal, AbortSignal.timeout(10_000)])
        : AbortSignal.timeout(10_000)
    }).then((response) => response.json())
    signal?.throwIfAborted()
    socket = new WebSocket(tab.webSocketDebuggerUrl)
    await new Promise((resolve, reject) => {
      const abort = () => {
        clearTimeout(timer)
        socket.close()
        reject(signal.reason)
      }
      signal?.addEventListener('abort', abort, { once: true })
      const cleanup = () => {
        clearTimeout(timer)
        signal?.removeEventListener('abort', abort)
      }
      const timer = setTimeout(() => {
        cleanup()
        socket.close()
        reject(new Error('Capture DevTools socket timed out'))
      }, 10_000)
      socket.addEventListener(
        'open',
        () => {
          cleanup()
          resolve()
        },
        { once: true }
      )
      socket.addEventListener(
        'error',
        (error) => {
          cleanup()
          reject(error)
        },
        { once: true }
      )
    })
    let serial = 0
    const abort = () => {
      for (const reject of pending.values()) reject(signal.reason)
    }
    signal?.addEventListener('abort', abort, { once: true })
    const session = {
      runtimeErrors: [],
      close: async () => {
        signal?.removeEventListener('abort', abort)
        await close()
      }
    }
    socket.addEventListener('message', (event) => {
      const message = JSON.parse(event.data)
      if (message.id) {
        pending.get(message.id)?.(undefined, message)
        pending.delete(message.id)
      } else if (message.method === 'Runtime.exceptionThrown')
        session.runtimeErrors.push(
          message.params.exceptionDetails.exception?.description ??
            message.params.exceptionDetails.text
        )
    })
    session.send = (method, params = {}) =>
      new Promise((resolve, reject) => {
        if (signal?.aborted) return reject(signal.reason)
        const id = ++serial
        const timeout = setTimeout(() => {
          pending.delete(id)
          reject(new CaptureTimeout('CDP timeout: ' + method))
        }, 30000)
        pending.set(id, (error, message) => {
          clearTimeout(timeout)
          if (error) {
            pending.delete(id)
            reject(error)
            return
          }
          if (message.error) reject(new Error(JSON.stringify(message.error)))
          else resolve(message.result)
        })
        socket.send(JSON.stringify({ id, method, params }))
      })
    session.evaluate = async (expression) => {
      const result = await session.send('Runtime.evaluate', {
        expression,
        awaitPromise: true,
        returnByValue: true
      })
      if (result.exceptionDetails)
        throw new Error(
          result.exceptionDetails.exception?.description ?? result.exceptionDetails.text
        )
      return result.result?.value
    }
    await session.send('Page.enable')
    await session.send('Runtime.enable')
    await session.send('Emulation.setDefaultBackgroundColorOverride', {
      color: { r: 0, g: 0, b: 0, a: 0 }
    })
    return session
  } catch (error) {
    await cleanupCapture(error, close)
    throw error
  }
}

export { findBrowser, openSession }
