import { copyFile, mkdir, mkdtemp, readdir, readFile, rm, stat, writeFile } from 'node:fs/promises'
import { createServer, type Server } from 'node:http'
import type { AddressInfo } from 'node:net'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { build } from 'vite'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import { resolvePageUrl, resolveReviewUrl } from '../figma/review/src/review/review-url'

// The Review is built once for the root, once with a relative base and once under a CDN prefix. Each build is
// served the way its base expects, and every URL the app would compute is requested.
const root = path.resolve(import.meta.dirname, '..')
const review = path.join(root, 'figma/review')
const publicSource = path.join(review, 'public/dcl-ui-review')

// The folder the Review is served from, named apart so no path-shaped literal of it is spelled out here.
const FOLDER = 'dcl-ui-review'
const CDN_PREFIX = '/@dcl/one-ui/1.2.3/'
const cases = [
  { name: 'the CDN prefix', base: CDN_PREFIX, mount: CDN_PREFIX },
  { name: 'a relative base', base: './', mount: '/anywhere/deep/' },
  { name: 'the root', base: '/', mount: '/' }
]

const types: Record<string, string> = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.json': 'application/json',
  '.png': 'image/png',
  '.svg': 'image/svg+xml'
}

let scratch: string
let publicDir: string
let reference: string
let render: string
const builds = new Map<string, string>()

async function buildReview(base: string, outDir: string) {
  await build({
    configFile: path.join(review, 'review.vite.config.ts'),
    base,
    logLevel: 'silent',
    publicDir,
    build: { outDir, emptyOutDir: true }
  })
}

// Serves outDir under `mount` only: anything outside it is a 404, as it would be on a CDN.
async function serve(outDir: string, mount: string): Promise<{ server: Server; origin: string }> {
  const server = createServer((request, response) => {
    void (async () => {
      const url = new URL(request.url ?? '/', 'http://localhost')
      if (!url.pathname.startsWith(mount)) return void response.writeHead(404).end()
      let file = path.join(outDir, decodeURIComponent(url.pathname.slice(mount.length)))
      if (!file.startsWith(outDir)) return void response.writeHead(403).end()
      try {
        if ((await stat(file)).isDirectory()) file = path.join(file, 'index.html')
        const body = await readFile(file)
        response
          .writeHead(200, { 'content-type': types[path.extname(file)] ?? 'text/plain' })
          .end(body)
      } catch {
        response.writeHead(404).end()
      }
    })()
  })
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
  return { server, origin: `http://127.0.0.1:${(server.address() as AddressInfo).port}` }
}

async function files(directory: string): Promise<string[]> {
  const found: string[] = []
  for (const entry of await readdir(directory, { withFileTypes: true, recursive: true }))
    if (entry.isFile()) found.push(path.join(entry.parentPath, entry.name))
  return found
}

beforeAll(async () => {
  scratch = await mkdtemp(path.join(tmpdir(), 'review-base-'))
  // A small public directory with one reference, one render and the fidelity report, in the logical layout.
  publicDir = path.join(scratch, 'public')
  const references = JSON.parse(
    await readFile(path.join(review, 'src/review/references.json'), 'utf8')
  ) as Record<string, { file: string }>
  const fidelity = JSON.parse(await readFile(path.join(publicSource, 'fidelity.json'), 'utf8')) as {
    cases: Record<string, { image?: string }>
  }
  reference = Object.values(references)[0]!.file
  render = Object.values(fidelity.cases).find((item) => item.image)!.image!
  for (const logical of [reference, render, '/dcl-ui-review/fidelity.json']) {
    const target = path.join(publicDir, logical)
    await mkdir(path.dirname(target), { recursive: true })
    await copyFile(path.join(review, 'public', logical), target)
  }
  for (const item of cases) {
    const outDir = path.join(scratch, `out-${cases.indexOf(item)}`)
    await buildReview(item.base, outDir)
    builds.set(item.name, outDir)
  }
}, 300_000)

afterAll(async () => {
  await rm(scratch, { recursive: true, force: true })
})

describe.each(cases)('a Review build for $name', ({ name, base, mount }) => {
  it('should hard-code no logical path in the built scripts or pages beyond the logical data', async () => {
    const outDir = builds.get(name)!
    // At the root base the pages legitimately name /dcl-ui-review/ themselves; under any other base they must not.
    const built = (await files(outDir)).filter((file) =>
      (base === '/' ? /\.js$/ : /\.(js|html)$/).test(file)
    )

    expect(built.length).toBeGreaterThan(0)
    for (const file of built) {
      const text = await readFile(file, 'utf8')
      // The data files keep logical paths, and the helper holds the logical root; nothing else may.
      const rest = text
        .replace(/\/dcl-ui-review\/(?:icons|references|renders)\//g, '')
        .replace(/["'`]\/dcl-ui-review\/["'`]/g, '')
      expect(rest, path.relative(outDir, file)).not.toMatch(/["'`(=]\/dcl-ui-review\//)
    }
  })

  it('should serve the index, the entry, fidelity.json, a reference and a render where the base puts them', async () => {
    const outDir = builds.get(name)!
    const { server, origin } = await serve(outDir, mount)
    try {
      const page = new URL(`${mount}${FOLDER}/index.html`, origin)
      const html = await (await fetch(page)).text()
      expect((await fetch(page)).status).toBe(200)
      const entry = new URL(/<script[^>]*src="([^"]+)"/.exec(html)![1]!, page)
      const stylesheet = new URL(
        /<link[^>]*rel="stylesheet"[^>]*href="([^"]+)"/.exec(html)![1]!,
        page
      )
      const moduleUrl = entry.href
      const appBase = base === '/' ? '/' : base === './' ? './' : base
      const requested = [
        entry.href,
        stylesheet.href,
        // The URLs the app computes at runtime, from the entry script's own URL for a relative base.
        resolveReviewUrl('/dcl-ui-review/fidelity.json', appBase, moduleUrl),
        resolveReviewUrl(reference, appBase, moduleUrl),
        resolveReviewUrl(render, appBase, moduleUrl)
      ].map((address) => new URL(address, entry).href)

      for (const address of requested) expect((await fetch(address)).status, address).toBe(200)
      expect(entry.pathname.startsWith(mount)).toBe(true)
      if (base !== '/')
        expect(await fetch(new URL('/dcl-ui-review/fidelity.json', origin))).toHaveProperty(
          'status',
          404
        )
    } finally {
      server.close()
    }
  })

  it('should serve the gallery links the app builds, and keep them under the mount', async () => {
    const outDir = builds.get(name)!
    const { server, origin } = await serve(outDir, mount)
    try {
      const page = new URL(`${mount}${FOLDER}/index.html`, origin)
      const html = await (await fetch(page)).text()
      const entry = new URL(/<script[^>]*src="([^"]+)"/.exec(html)![1]!, page)
      const queries = [
        'platform=all&component=NavBar&variant=0',
        'platform=web&component=Button&variant=3',
        'platform=all&component=NavBar&variant=0&foundations=icons'
      ]

      for (const query of queries) {
        const link = new URL(resolvePageUrl(query, base, entry.href), page)
        expect(link.pathname.startsWith(mount), link.href).toBe(true)
        expect(link.search).toBe(`?${query}`)
        expect((await fetch(link)).status, link.href).toBe(200)
      }
    } finally {
      server.close()
    }
  })

  // The page at the output root is the same document one level up. Under a relative base, left as emitted its
  // ../dcl-ui-review/ URLs would resolve outside the folder the app is mounted at.
  it('should keep every URL of the root index.html under the mount, and each one served', async () => {
    const outDir = builds.get(name)!
    const { server, origin } = await serve(outDir, mount)
    try {
      const rootPage = new URL(`${mount}index.html`, origin)
      const response = await fetch(rootPage)
      const html = await response.text()
      expect(response.status).toBe(200)
      const references = [
        ...html.matchAll(/<(?:script|link|img|source)\b[^>]*?\b(?:src|href)="([^"]+)"/g)
      ].map((match) => match[1]!)

      expect(references.some((reference) => /\.js(?:\?|$)/.test(reference))).toBe(true)
      expect(references.some((reference) => /\.css(?:\?|$)/.test(reference))).toBe(true)
      for (const reference of references) {
        const url = new URL(reference, rootPage)
        expect(url.origin, reference).toBe(origin)
        expect(url.pathname.startsWith(mount), `${reference} -> ${url.pathname}`).toBe(true)
        expect((await fetch(url)).status, url.href).toBe(200)
      }
    } finally {
      server.close()
    }
  })
})

describe('a Review build for the root', () => {
  it('should leave the root index.html exactly as the build emitted it, as before the base was configurable', async () => {
    const outDir = builds.get('the root')!

    expect(await readFile(path.join(outDir, 'index.html'), 'utf8')).toBe(
      await readFile(path.join(outDir, FOLDER, 'index.html'), 'utf8')
    )
  })
})

describe('a Review build, whatever its base', () => {
  it('should write the sealed inputs unchanged: the logical data files are not rewritten', async () => {
    const built = builds.get('the CDN prefix')!
    const logical = await readFile(path.join(publicDir, '/dcl-ui-review/fidelity.json'), 'utf8')

    expect(await readFile(path.join(built, FOLDER, 'fidelity.json'), 'utf8')).toBe(logical)
    expect(logical).toContain('"/dcl-ui-review/renders/')
  })

  it('should leave the sources of the sealed Review inputs untouched', async () => {
    const references = await readFile(path.join(review, 'src/review/references.json'), 'utf8')

    expect(references).toContain('"/dcl-ui-review/references/')
    await writeFile(path.join(scratch, 'ok'), 'ok')
  })
})
