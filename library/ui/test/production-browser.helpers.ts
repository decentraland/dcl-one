import path from 'node:path'
import process from 'node:process'
import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { createServer, type Server } from 'node:http'
import type { AddressInfo } from 'node:net'
import { tmpdir } from 'node:os'
import { build } from 'vite'
import { afterAll, beforeAll } from 'vitest'
import { findBrowser, openSession } from '../figma/review/scripts/capture-browser.mjs'

export function productionBrowser(entry: string) {
  let scratch: string
  let server: Server
  let origin: string
  let session: Awaited<ReturnType<typeof openSession>>
  async function evaluate<T>(expression: string): Promise<T> {
    const result = (await session.send('Runtime.evaluate', {
      expression,
      awaitPromise: true,
      returnByValue: true
    })) as { result: { value: T }; exceptionDetails?: unknown }
    if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails))
    return result.result.value
  }
  beforeAll(async () => {
    const root = path.resolve(import.meta.dirname, '..')
    scratch = await mkdtemp(path.join(tmpdir(), 'native-ui-browser-'))
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
    await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
    origin = `http://127.0.0.1:${(server.address() as AddressInfo).port}/`
    session = await openSession(
      await findBrowser([
        process.env.BROWSER_PATH,
        process.env.CHROME_PATH,
        '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
        '/usr/bin/google-chrome',
        '/usr/bin/chromium',
        '/usr/bin/chromium-browser'
      ])
    )
    await session.send('Emulation.setDeviceMetricsOverride', {
      width: 1440,
      height: 900,
      deviceScaleFactor: 1,
      mobile: false
    })
  }, 30000)
  afterAll(async () => {
    try {
      await session?.close()
    } finally {
      if (server?.listening)
        await new Promise<void>((resolve, reject) =>
          server.close((error) => (error ? reject(error) : resolve()))
        )
      if (scratch) await rm(scratch, { recursive: true, force: true })
    }
  })

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
