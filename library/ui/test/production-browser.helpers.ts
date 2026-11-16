import path from 'node:path'
import process from 'node:process'
import { mkdtemp, readFile } from 'node:fs/promises'
import { createServer, type Server } from 'node:http'
import type { AddressInfo } from 'node:net'
import { tmpdir } from 'node:os'
import { build } from 'vite'
import { afterAll, beforeAll } from 'vitest'
import { releaseHarness } from './release-harness'
import { findBrowser, openSession } from '../figma/review/scripts/capture-browser.mjs'

type FixtureHooks = {
  beforeAll: (setup: () => Promise<void>, timeout: number) => void
  afterAll: (cleanup: () => Promise<void>, timeout: number) => void
}
export function productionBrowser(entry: string, hooks: FixtureHooks = { beforeAll, afterAll }) {
  let scratch: string
  let server: Server
  let origin: string
  let session: Awaited<ReturnType<typeof openSession>>
  let preparation: Promise<void> | undefined
  let startup: ReturnType<typeof openSession> | undefined
  let releasing: Promise<void> | undefined
  let detachServerErrors: (() => void) | undefined
  const controller = new AbortController()
  async function evaluate<T>(expression: string): Promise<T> {
    const result = (await session.send('Runtime.evaluate', {
      expression,
      awaitPromise: true,
      returnByValue: true
    })) as { result: { value: T }; exceptionDetails?: unknown }
    if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails))
    return result.result.value
  }
  hooks.beforeAll(() => {
    preparation ??= prepareHarness()
    return preparation
  }, 30_000)
  async function prepareHarness() {
    controller.signal.throwIfAborted()
    const root = path.resolve(import.meta.dirname, '..')
    scratch = await mkdtemp(path.join(tmpdir(), 'native-ui-browser-'))
    controller.signal.throwIfAborted()
    const environment = process.env.NODE_ENV
    process.env.NODE_ENV = 'production'
    try {
      await build({
        configFile: false,
        define: { 'process.env.NODE_ENV': JSON.stringify('production') },
        root,
        logLevel: 'silent',
        resolve: {
          dedupe: ['react', 'react-dom'],
          alias: [
            { find: /^@dcl\/one-ui$/, replacement: path.join(root, 'index.ts') },
            {
              find: /^@dcl\/one-ui\/foundations$/,
              replacement: path.join(root, 'foundations/index.ts')
            }
          ]
        },
        build: {
          outDir: scratch,
          emptyOutDir: true,
          lib: {
            entry: path.join(root, entry),
            formats: ['es'],
            fileName: 'fixture',
            cssFileName: 'fixture'
          }
        }
      })
    } finally {
      if (environment === undefined) delete process.env.NODE_ENV
      else process.env.NODE_ENV = environment
    }

    controller.signal.throwIfAborted()
    server = createServer((request, response) => {
      void (async () => {
        const pathname = new URL(request.url ?? '/', 'http://localhost').pathname
        if (pathname === '/') {
          response
            .writeHead(200, { 'content-type': 'text/html' })
            .end(
              '<!doctype html><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/fixture.css"><div id="root"></div><script type="module" src="/fixture.js"></script>'
            )
          return
        }
        const type =
          pathname === '/fixture.js'
            ? 'text/javascript'
            : pathname === '/fixture.css'
              ? 'text/css'
              : undefined
        if (!type) {
          response.writeHead(404).end()
          return
        }
        try {
          const bytes = await readFile(path.join(scratch, pathname.slice(1)))
          response.writeHead(200, { 'content-type': type }).end(bytes)
        } catch {
          response.writeHead(404).end()
        }
      })()
    })
    await new Promise<void>((resolve, reject) => {
      let settled = false
      const finish = (error?: unknown) => {
        if (settled) return
        settled = true
        controller.signal.removeEventListener('abort', onAbort)
        if (error !== undefined) reject(error)
        else resolve()
      }
      const onAbort = () => finish(controller.signal.reason)
      const onError = (error: Error) => finish(error)
      // A queued bind error after abort still belongs to this server until its close event.
      detachServerErrors = () => {
        server.removeListener('error', onError)
        server.removeListener('close', detachServerErrors!)
      }
      server.on('error', onError)
      server.once('close', detachServerErrors)
      controller.signal.addEventListener('abort', onAbort, { once: true })
      if (controller.signal.aborted) onAbort()
      else {
        try {
          server.listen({ port: 0, host: '127.0.0.1', signal: controller.signal }, () => finish())
        } catch (error) {
          finish(error)
        }
      }
    })
    controller.signal.throwIfAborted()
    origin = `http://127.0.0.1:${(server.address() as AddressInfo).port}/`
    startup = (async () => {
      const executable = await findBrowser([
        process.env.BROWSER_PATH,
        process.env.CHROME_PATH,
        '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
        '/usr/bin/google-chrome',
        '/usr/bin/chromium',
        '/usr/bin/chromium-browser'
      ])
      controller.signal.throwIfAborted()
      return openSession(executable, { signal: controller.signal })
    })()
    session = await startup
    controller.signal.throwIfAborted()
    await session.send('Emulation.setDeviceMetricsOverride', {
      width: 1440,
      height: 900,
      deviceScaleFactor: 1,
      mobile: false
    })
  }
  hooks.afterAll(() => {
    releasing ??= (async () => {
      controller.abort(new Error('Production browser fixture disposed'))
      await preparation?.catch(() => undefined)
      const opened = await startup?.catch(() => undefined)
      try {
        await releaseHarness({ session: opened, server, scratch })
      } finally {
        detachServerErrors?.()
      }
    })()
    return releasing
  }, 30_000)

  return {
    evaluate,
    get session() {
      return session
    },
    get origin() {
      return origin
    }
  }
}
