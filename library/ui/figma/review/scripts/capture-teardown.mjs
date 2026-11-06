import process from 'node:process'
import { rm } from 'node:fs/promises'
import { setTimeout, clearTimeout } from 'node:timers'
import { setTimeout as sleep } from 'node:timers/promises'

// What a Review capture stops when it ends or restarts its browser. It lives in a module, not in the capture
// script, because that script runs on import and nothing could reach its teardown to test it.

// The DevTools socket is closed, then the browser is asked to stop. A hung browser can ignore SIGTERM, and
// teardown must never hide the capture error behind the job timeout, so SIGKILL follows after graceMs. A
// browser that has already exited returns at once.
async function stopBrowser({ socket, browser, graceMs = 5000 }) {
  socket?.close()
  if (!browser.pid) return
  browser.kill('SIGTERM')
  await new Promise((resolve) => {
    if (browser.exitCode !== null || browser.signalCode !== null) return resolve()
    const kill = setTimeout(() => browser.kill('SIGKILL'), graceMs)
    browser.once('exit', () => {
      clearTimeout(kill)
      resolve()
    })
  })
}

// Keep-alive connections from the browser would otherwise keep the process running: close() only stops
// listening, and closeAllConnections() ends the sockets a request is still using.
function closeServer(server) {
  server.close()
  server.closeAllConnections()
}

// Browser helper processes can still write to the profile for a moment after the browser exits, so a busy or
// not-yet-empty profile is retried (10 times, 200 ms apart). After that budget the failure is raised, never
// swallowed; any other error is raised at once.
const TRANSIENT = new Set(['EBUSY', 'ENOTEMPTY', 'EPERM', 'EMFILE', 'ENFILE'])

async function removeProfile(profile, { remove = rm, retries = 10, delay = 200 } = {}) {
  for (let attempt = 0; ; attempt += 1) {
    try {
      return await remove(profile, { recursive: true, force: true })
    } catch (error) {
      if (!TRANSIENT.has(error?.code)) throw error
      if (attempt === retries)
        throw new Error(
          `Could not remove the capture profile after ${retries + 1} attempts (${error.code}): ${profile}`,
          { cause: error }
        )
      await sleep(delay)
    }
  }
}

export { closeServer, removeProfile, stopBrowser }

// Preserve the capture failure if cleanup also fails, while still surfacing the cleanup diagnostic.
async function cleanupCapture(primary, cleanup, log = globalThis.console.error) {
  try {
    await cleanup()
  } catch (error) {
    if (!primary) throw error
    log('Capture cleanup failed:', error)
  }
}

function installCaptureSignals(controller, target = process) {
  const handlers = new Map(
    ['SIGINT', 'SIGTERM'].map((name, index) => {
      const handler = () => {
        target.exitCode = index === 0 ? 130 : 143
        if (!controller.signal.aborted)
          controller.abort(new Error(`Capture interrupted by ${name}`))
      }
      target.once(name, handler)
      return [name, handler]
    })
  )
  return () => {
    for (const [name, handler] of handlers) target.removeListener(name, handler)
  }
}
export { cleanupCapture, installCaptureSignals }
