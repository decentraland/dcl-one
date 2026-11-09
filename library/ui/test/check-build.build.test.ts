import { cp, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import { createServer, type Server } from 'node:http'
import { tmpdir } from 'node:os'
import path from 'node:path'
import process from 'node:process'
import { build } from 'vite'
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from 'vitest'
import { checkBuild } from '../figma/review/scripts/check-build.mjs'

// The gallery's functional proof, run against a real relative-base build served under a nested prefix, and
// against broken copies of it: each must fail the check rather than pass vacuously.
const review = path.resolve(import.meta.dirname, '../figma/review')
let scratch: string
let dist: string

beforeAll(async () => {
  scratch = await mkdtemp(path.join(tmpdir(), 'check-build-'))
  dist = path.join(scratch, 'dist')
  // The runner sets NODE_ENV=test, which would bundle React's development build; publish what CI publishes.
  const environment = process.env.NODE_ENV
  process.env.NODE_ENV = 'production'
  try {
    await build({
      configFile: path.join(review, 'review.vite.config.ts'),
      mode: 'production',
      base: './',
      logLevel: 'silent',
      build: { outDir: dist, emptyOutDir: true }
    })
  } finally {
    process.env.NODE_ENV = environment
  }
}, 300_000)

afterAll(() => rm(scratch, { recursive: true, force: true }))

async function brokenCopy(name: string, breakIt: (copy: string) => Promise<void>) {
  const copy = path.join(scratch, name)
  await cp(dist, copy, { recursive: true })
  await breakIt(copy)
  return copy
}

describe('when checking a built gallery', () => {
  it('should navigate the catalog, switch variants and draw an audit render under a nested prefix', async () => {
    const summary = await checkBuild({ dist, prefix: '/nested/prefix/' })

    expect(summary.entry).toBe('/nested/prefix/index.html')
    expect(summary.components).toBeGreaterThan(0)
    expect(summary.variant).toBeGreaterThan(0)
    expect(summary.audit.width).toBeGreaterThan(0)
    expect(summary.audit.height).toBeGreaterThan(0)
  }, 120_000)

  it('should also work at the root', async () => {
    await expect(checkBuild({ dist })).resolves.toMatchObject({ entry: '/index.html' })
  }, 120_000)

  it('should fail when a script the entry loads is missing', async () => {
    const copy = await brokenCopy('missing-script', async (target) => {
      const assets = path.join(target, 'dcl-ui-review/assets')
      const script = (await readdir(assets)).find((file) => file.endsWith('.js'))!
      await rm(path.join(assets, script))
    })

    await expect(checkBuild({ dist: copy, prefix: '/nested/prefix/' })).rejects.toThrow()
  }, 120_000)

  it('should fail when the root entry is missing', async () => {
    const copy = await brokenCopy('missing-entry', (target) => rm(path.join(target, 'index.html')))

    await expect(checkBuild({ dist: copy })).rejects.toThrow(/Timed out|requests failed/)
  }, 120_000)

  it('should fail on an uncaught page error', async () => {
    const copy = await brokenCopy('page-error', async (target) => {
      const assets = path.join(target, 'dcl-ui-review/assets')
      await writeFile(path.join(assets, 'boom.js'), 'throw new Error("boom")\n')
      const entry = path.join(target, 'index.html')
      const html = await readFile(entry, 'utf8')
      await writeFile(
        entry,
        html.replace('</head>', '<script src="./dcl-ui-review/assets/boom.js"></script></head>')
      )
    })

    await expect(checkBuild({ dist: copy })).rejects.toThrow(/page error|boom/)
  }, 120_000)

  describe('and a successful request escapes to a second controlled origin', () => {
    let external: Server
    let origin: string
    let requests: string[]
    let copy: string

    beforeEach(async () => {
      requests = []
      external = createServer((request, response) => {
        requests.push(request.url ?? '/')
        response.setHeader('access-control-allow-origin', '*')
        if (request.url === '/escape.js')
          response.writeHead(200, { 'content-type': 'text/javascript' }).end('void 0')
        else if (request.url === '/escape.svg')
          response
            .writeHead(200, { 'content-type': 'image/svg+xml' })
            .end('<svg xmlns="http://www.w3.org/2000/svg"/>')
        else
          response
            .writeHead(200, { 'content-type': 'text/html' })
            .end('<!doctype html><title>External document</title>')
      })
      await new Promise<void>((resolve) => external.listen(0, '127.0.0.1', resolve))
      origin = `http://127.0.0.1:${(external.address() as { port: number }).port}`
    })

    afterEach(async () => {
      external.closeAllConnections()
      await new Promise<void>((resolve, reject) =>
        external.close((error) => (error ? reject(error) : resolve()))
      )
    })

    describe.each([
      ['worker', 'escape-worker', ''],
      ['script', 'escape.js', '<script src="URL"></script>'],
      ['image', 'escape.svg', '<img src="URL" alt="External fixture" />'],
      ['document', 'escape.html', '<iframe src="URL" title="External fixture"></iframe>']
    ])('and the escaped resource type is %s', (kind, file, markup) => {
      beforeEach(async () => {
        copy = await brokenCopy(`external-${kind}`, async (target) => {
          const entry = path.join(target, 'index.html')
          const html = await readFile(entry, 'utf8')
          if (kind === 'worker') {
            const module = /<script[^>]*type="module"[^>]*src="([^"]+)"[^>]*><\/script>/.exec(html)
            if (!module) throw new Error('The fixture has no module entry')
            await writeFile(
              path.join(target, 'worker.js'),
              `fetch(${JSON.stringify(`${origin}/${file}`)}).then((response) => response.text()).then(() => postMessage('done'))`
            )
            // Wait for the worker before mounting the gallery, so the request cannot race the proof window.
            await writeFile(
              entry,
              html.replace(
                module[0],
                `<script type="module">
              const worker = new Worker(new URL('./worker.js', location.href));
              await new Promise((resolve, reject) => { worker.onmessage = resolve; worker.onerror = reject });
              worker.terminate();
              await import(${JSON.stringify(module[1])});
            </script>`
              )
            )
            return
          }
          await writeFile(
            entry,
            html.replace('</body>', `${markup.replace('URL', `${origin}/${file}`)}</body>`)
          )
        })
      })

      it('should reject the external URL even though it responds with HTTP 200', async () => {
        await expect(checkBuild({ dist: copy, prefix: '/nested/prefix/' })).rejects.toThrow(
          /request left the.*(origin|prefix)/
        )
        expect(requests).toContain(`/${file}`)
      }, 120_000)
    })

    it('should name the second origin when the entry redirects there and it responds with HTTP 200', async () => {
      copy = await brokenCopy('redirect-external', async (target) => {
        const entry = path.join(target, 'index.html')
        const html = await readFile(entry, 'utf8')
        await writeFile(
          entry,
          html.replace(
            '<head>',
            `<head><script>location.replace(${JSON.stringify(`${origin}/landed.html`)})</script>`
          )
        )
      })

      // The entry can finish loading before the redirect commits, so either the request or the final document names it.
      const escaped = `${origin}/landed.html`.replaceAll('.', '\\.')
      await expect(checkBuild({ dist: copy, prefix: '/nested/prefix/' })).rejects.toThrow(
        new RegExp(`(request|document) left the expected origin or prefix: ${escaped}$`)
      )
      expect(requests).toContain('/landed.html')
    }, 120_000)
  })

  it('should fail when the page moves its document outside the prefix without a request', async () => {
    const copy = await brokenCopy('history-outside', async (target) => {
      const entry = path.join(target, 'index.html')
      const html = await readFile(entry, 'utf8')
      await writeFile(
        entry,
        html.replace(
          '<head>',
          `<head><script>addEventListener('load', () => history.replaceState(null, '', '/outside/landed.html'))</script>`
        )
      )
    })

    await expect(checkBuild({ dist: copy, prefix: '/nested/prefix/' })).rejects.toThrow(
      /the document left the expected origin or prefix: http:\/\/127\.0\.0\.1:\d+\/outside\/landed\.html/
    )
  }, 120_000)

  describe('and a teardown step fails', () => {
    let origin: string
    let serverCloses: number
    // A stand-in session that records the served origin and answers each check the way `evaluate` does.
    const session =
      (evaluate: (expression: string) => unknown, closes = true) =>
      async () => ({
        requests: [],
        requestFailures: [],
        runtimeErrors: [],
        consoleErrors: [],
        send: async (method: string, params?: Record<string, unknown>) => {
          if (method === 'Page.navigate') origin ||= new URL(String(params?.url)).origin
          return {}
        },
        evaluate: async (expression: string) => evaluate(expression),
        close: async () => {
          if (!closes) throw new Error('browser would not close')
        }
      })
    // Answers every check the way a working gallery does.
    const passing = (expression: string) => {
      const page = `${origin}/?component=Button`
      if (
        expression === 'location.href' ||
        expression.includes("readyState === 'complete' && location.href")
      )
        return page
      if (expression.includes('component=')) return [{ href: page, name: 'Button' }]
      if (expression.includes('main h1')) return true
      if (expression.includes('select.dispatchEvent')) return { property: 'size', value: 'large' }
      if (expression.includes("searchParams.get('variant')")) return '1'
      if (expression.includes('#audit-render')) return { width: 10, height: 10 }
      throw new Error(`unexpected expression: ${expression}`)
    }
    const unreachable = () => {
      throw new Error('page unreachable')
    }
    // A real server that does close, but whose close callback reports an error.
    const failingServer: typeof createServer = ((...args: Parameters<typeof createServer>) => {
      const server = createServer(...args)
      const close = server.close.bind(server)
      server.close = (callback) => {
        serverCloses++
        return close(() => callback?.(new Error('server would not close')))
      }
      return server
    }) as typeof createServer

    beforeEach(() => {
      origin = ''
      serverCloses = 0
    })

    it('should keep the check failure when the browser fails to close and still close the server', async () => {
      await expect(
        checkBuild({ dist, browser: 'unused', open: session(unreachable, false) })
      ).rejects.toThrow('page unreachable')
      expect(origin).not.toBe('')
      await expect(fetch(origin)).rejects.toThrow()
    })

    it('should report the browser close failure after a passing check and still close the server', async () => {
      await expect(
        checkBuild({ dist, browser: 'unused', open: session(passing, false) })
      ).rejects.toThrow('browser would not close')
      await expect(fetch(origin)).rejects.toThrow()
    })

    it('should report the server close failure after a passing check', async () => {
      await expect(
        checkBuild({
          dist,
          browser: 'unused',
          open: session(passing),
          createServer: failingServer
        })
      ).rejects.toThrow('server would not close')
      expect(serverCloses).toBe(1)
    })

    it('should report the check failure when the server also fails to close', async () => {
      await expect(
        checkBuild({
          dist,
          browser: 'unused',
          open: session(unreachable),
          createServer: failingServer
        })
      ).rejects.toThrow('page unreachable')
      expect(serverCloses).toBe(1)
    })

    it('should report the browser close failure when the server also fails to close, after attempting both', async () => {
      await expect(
        checkBuild({
          dist,
          browser: 'unused',
          open: session(passing, false),
          createServer: failingServer
        })
      ).rejects.toThrow('browser would not close')
      expect(serverCloses).toBe(1)
      await expect(fetch(origin)).rejects.toThrow()
    })
  })

  it('should reject a prefix that is not an absolute path', async () => {
    await expect(checkBuild({ dist, prefix: 'relative/' })).rejects.toThrow('absolute URL path')
  })
})
