import { mkdtemp, readFile, stat } from 'node:fs/promises'
import { createServer, type Server } from 'node:http'
import type { AddressInfo } from 'node:net'
import { tmpdir } from 'node:os'
import path from 'node:path'
import process from 'node:process'
import { build } from 'vite'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import { findBrowser, openSession } from '../figma/review/scripts/capture-browser.mjs'
import { releaseHarness } from './release-harness'

// Every NavBar combination, rendered by the built Review in Chromium at its captured size: no descendant may extend
// past the root's box. A CSS gap under space-between distribution used to push the controls out of the nav.
const root = path.resolve(import.meta.dirname, '..')
const review = path.join(root, 'figma/review')
// The folder the Review is served from, named apart so no path-shaped literal of it is spelled out here.
const FOLDER = 'dcl-ui-review'
// Every NavBar combination in the built catalog, so a variant added later is measured too.
const catalog = JSON.parse(
  await readFile(path.join(review, 'src/review/catalog.json'), 'utf8')
) as {
  components: { name: string; variants: unknown[] }[]
}
const VARIANTS = catalog.components.find((component) => component.name === 'NavBar')!.variants
  .length
const TOLERANCE = 1

const types: Record<string, string> = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.json': 'application/json',
  '.svg': 'image/svg+xml',
  '.woff2': 'font/woff2'
}

let scratch: string
let server: Server
let origin: string
let session: Awaited<ReturnType<typeof openSession>>

beforeAll(async () => {
  scratch = await mkdtemp(path.join(tmpdir(), 'navbar-bounds-'))
  const outDir = path.join(scratch, 'out')
  await build({
    configFile: path.join(review, 'review.vite.config.ts'),
    base: '/',
    logLevel: 'silent',
    build: { outDir, emptyOutDir: true }
  })
  server = createServer((request, response) => {
    void (async () => {
      let file = path.join(
        outDir,
        decodeURIComponent(new URL(request.url ?? '/', 'http://localhost').pathname)
      )
      if (!file.startsWith(outDir)) return void response.writeHead(403).end()
      try {
        if ((await stat(file)).isDirectory()) file = path.join(file, 'index.html')
        response
          .writeHead(200, { 'content-type': types[path.extname(file)] ?? 'text/plain' })
          .end(await readFile(file))
      } catch {
        response.writeHead(404).end()
      }
    })()
  })
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
  origin = `http://127.0.0.1:${(server.address() as AddressInfo).port}`
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
  await session.send('Page.enable')
}, 300_000)

// Setup can fail before the server or browser exist (a Vite build failure); release only what started.
afterAll(() => releaseHarness({ session, server, scratch }))

type Bounds = {
  width: number
  height: number
  overflow: { name: string; right: number; bottom: number }[]
}

async function measure(variant: number): Promise<Bounds> {
  await session.navigate(`${origin}/${FOLDER}/?audit=1&component=NavBar&variant=${variant}`)
  const { result, exceptionDetails } = (await session.send('Runtime.evaluate', {
    expression: `new Promise((resolve, reject) => {
      const started = Date.now()
      const poll = () => {
        const root = document.querySelector('#audit-render')?.firstElementChild
        if (!root) return Date.now() - started > 15000 ? reject(new Error('Rendered root missing')) : setTimeout(poll, 50)
        document.fonts.ready.then(() => requestAnimationFrame(() => requestAnimationFrame(() => {
          const box = root.getBoundingClientRect()
          const overflow = [...root.querySelectorAll('*')].map((element) => {
            const r = element.getBoundingClientRect()
            return { name: element.getAttribute('data-n') || element.tagName, right: r.right - box.right, bottom: r.bottom - box.bottom, left: box.left - r.left, top: box.top - r.top, w: r.width, h: r.height }
          }).filter((item) => item.w > 0 && item.h > 0 && (item.right > ${TOLERANCE} || item.bottom > ${TOLERANCE} || item.left > ${TOLERANCE} || item.top > ${TOLERANCE}))
          resolve({ width: box.width, height: box.height, overflow: overflow.map(({ name, right, bottom }) => ({ name, right, bottom })) })
        })))
      }
      poll()
    })`,
    awaitPromise: true,
    returnByValue: true
  })) as {
    result: { value: Bounds }
    exceptionDetails?: { text: string; exception?: { description?: string } }
  }
  // A variant that throws while rendering must report its own error, not a missing measurement.
  if (exceptionDetails)
    throw new Error(exceptionDetails.exception?.description ?? exceptionDetails.text)
  return result.value
}

describe('every NavBar combination in the built Review', () => {
  it('should find the NavBar combinations in the catalog', () => {
    expect(VARIANTS).toBeGreaterThan(0)
  })

  it.each(Array.from({ length: VARIANTS }, (_, index) => index))(
    'variant %i keeps every descendant inside its root box',
    async (variant) => {
      const bounds = await measure(variant)
      expect(session.runtimeErrors).toEqual([])
      expect(bounds.width).toBeGreaterThan(0)
      expect(bounds.overflow).toEqual([])
    },
    60_000
  )
})
