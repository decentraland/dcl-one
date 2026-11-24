import { readFile } from 'node:fs/promises'
import { createServer, type Server } from 'node:http'
import path from 'node:path'
import process from 'node:process'
import { setTimeout as delay } from 'node:timers/promises'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import { findBrowser, openSession } from '../figma/review/scripts/capture-browser.mjs'
import { checkLayout } from '../gallery/scripts/check-build.mjs'

// The gallery build check's layout rule (gallery/scripts/check-build.mjs), run on canvases built to break it the
// ways a measure can miss: each broken page must fail, and its sound twin must pass, so neither result is vacuous.
const FONT = path.resolve(
  import.meta.dirname,
  '../foundations/typography/fonts/inter-latin-wght-normal.woff2'
)
// A 1x1 PNG, drawn at the size its attributes give.
const PIXEL =
  'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=='

const stage = (body: string, head = '') =>
  `<!doctype html><html><head><meta charset="utf-8">${head}<style>body{margin:0;font:16px sans-serif}</style></head>` +
  `<body><section data-example="case"><div data-stage>${body}</div></section></body></html>`

// A picture 30px taller than its 100px holder.
const spill = (overflow: string) =>
  stage(
    `<div style="width:200px;height:100px;${overflow}"><img src="${PIXEL}" width="200" height="130" alt=""></div>`
  )

// A word in a 100px box beside its neighbour: it fits in the fallback font and, with `scale`, overruns the neighbour
// once the web font arrives. The word takes the font only after the page has loaded, as a component that renders
// late does, so the load event does not wait for it, and the server holds the font back: the first measure sees the
// fallback.
const lateFont = (scale: string) =>
  stage(
    `<div style="display:flex;width:300px"><span id="word" style="flex:none;width:100px;white-space:nowrap">Wide</span><span>Neighbour</span></div>`,
    `<style>@font-face{font-family:Late;src:url(/late-font.woff2?${scale}) format("woff2");font-display:swap;size-adjust:${scale}}</style>` +
      `<script>addEventListener('load', () => { document.getElementById('word').style.fontFamily = 'Late, sans-serif' })</script>`
  )

// A HUD-like window that scrolls inside itself, 300px high: a sound stage fills the first 600px, and the stage under
// it, out of view until the window scrolls, holds the picture with `overflow`.
const offscreen = (overflow: string) =>
  `<!doctype html><html><head><meta charset="utf-8"><style>body{margin:0}</style></head><body>` +
  `<main style="height:300px;overflow-y:auto">` +
  `<section data-example="first"><div data-stage><div style="height:600px"></div></div></section>` +
  `<section data-example="below"><div data-stage><div style="width:200px;height:100px;${overflow}"><img src="${PIXEL}" width="200" height="130" alt=""></div></div></section>` +
  `</main></body></html>`

const canvases: Record<string, string> = {
  'window-offscreen-spill': offscreen('overflow-x:clip;overflow-y:visible'),
  'window-offscreen-clipped': offscreen('overflow:hidden'),
  // Covers the component positions, 60px taller than their 200x100 slot.
  'absolute-cover-spill': stage(
    `<div style="position:relative;width:200px;height:100px"><img src="${PIXEL}" width="200" height="160" alt="" style="position:absolute;top:0;left:0"></div>`
  ),
  'fixed-cover-spill': stage(
    `<div style="width:200px;height:100px"><img src="${PIXEL}" width="200" height="160" alt="" style="position:fixed;top:0;left:0"></div>`
  ),
  // A 14px badge the button positions 3px over its corner, as the HUD IconButton's corner art.
  'positioned-corner-badge': stage(
    `<button style="position:relative;width:42px;height:42px;padding:0;border:0"><img src="${PIXEL}" width="14" height="14" alt="" style="position:absolute;top:-3px;right:-3px"></button>`
  ),
  'clip-x-spill': spill('overflow-x:clip;overflow-y:visible'),
  'clip-both': spill('overflow:clip'),
  'hidden-both': spill('overflow:hidden'),
  'paint-contained': spill('contain:paint'),
  'late-font-overlap': lateFont('400%'),
  'late-font-fits': lateFont('100%')
}

let server: Server
let origin: string
let session: Awaited<ReturnType<typeof openSession>>

beforeAll(async () => {
  const font = await readFile(FONT)
  server = createServer((request, response) => {
    void (async () => {
      const url = new URL(request.url ?? '/', 'http://127.0.0.1')
      if (url.pathname === '/late-font.woff2') {
        await delay(1500)
        return void response.writeHead(200, { 'content-type': 'font/woff2' }).end(font)
      }
      const [, kind, name] = url.pathname.split('/')
      const canvas = name ? canvases[name] : undefined
      if (!canvas) return void response.writeHead(404).end()
      response.writeHead(200, { 'content-type': 'text/html' })
      response.end(
        kind === 'page'
          ? `<!doctype html><iframe src="/canvas/${name}" style="width:400px;height:400px;border:0"></iframe>`
          : canvas
      )
    })()
  })
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
  const address = server.address()
  origin = `http://127.0.0.1:${typeof address === 'object' && address ? address.port : 0}`
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
}, 30_000)

afterAll(async () => {
  try {
    await session?.close()
  } finally {
    server.closeAllConnections()
    await new Promise((resolve) => server.close(resolve))
  }
})

async function layoutOf(name: string) {
  await session.navigate(`${origin}/page/${name}`)
  return checkLayout(session, name)
}

describe('when the gallery check measures a canvas', () => {
  it('should fail a picture that spills below a holder clipping only its horizontal axis', async () => {
    await expect(layoutOf('clip-x-spill')).rejects.toThrow(
      /a picture paints 200x130 outside its 200x100 div/
    )
  })

  it.each(['clip-both', 'hidden-both', 'paint-contained'])(
    'should pass a picture its %s holder clips on both axes',
    async (name) => {
      await expect(layoutOf(name)).resolves.toBeUndefined()
    }
  )

  it('should scroll a window that scrolls inside itself and fail a spill out of view at first', async () => {
    await expect(layoutOf('window-offscreen-spill')).rejects.toThrow(
      /below: a picture paints 200x130 outside its 200x100 div \(scrolled \d+px\)/
    )
  })

  it('should pass the same window when the picture below stays in its slot', async () => {
    await expect(layoutOf('window-offscreen-clipped')).resolves.toBeUndefined()
  })

  it.each(['absolute', 'fixed'])(
    'should fail a cover with position %s that paints 60px past its slot',
    async (position) => {
      await expect(layoutOf(`${position}-cover-spill`)).rejects.toThrow(
        /a picture paints 200x160 outside its 200x100 div/
      )
    }
  )

  it('should pass a picture the component positions over the edge of its holder', async () => {
    await expect(layoutOf('positioned-corner-badge')).resolves.toBeUndefined()
  })

  it('should measure after a late web font loads and fail the overlap it causes', async () => {
    await expect(layoutOf('late-font-overlap')).rejects.toThrow(/"Wide" overlaps "Neighbour"/)
  }, 30_000)

  it('should pass the same page when the late font still fits', async () => {
    await expect(layoutOf('late-font-fits')).resolves.toBeUndefined()
  }, 30_000)
})
