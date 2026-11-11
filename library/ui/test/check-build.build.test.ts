import { cp, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import { createServer, type RequestListener, type Server } from 'node:http'
import { tmpdir } from 'node:os'
import path from 'node:path'
import process from 'node:process'
import { build } from 'vite'
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from 'vitest'
import { checkBuild } from '../figma/review/scripts/check-build.mjs'
import { openSession } from '../figma/review/scripts/capture-browser.mjs'

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

    describe.each(['document', 'module'] as const)(
      'and the %s entry responds with HTTP 302',
      (kind) => {
        let redirectServer: typeof createServer
        let redirected: boolean
        let destination: string

        beforeEach(async () => {
          redirected = false
          destination = origin + (kind === 'module' ? '/escape.js' : '/landed.html')
          const html = await readFile(path.join(dist, 'index.html'), 'utf8')
          const module = /<script[^>]*type="module"[^>]*src="([^"]+)"[^>]*><\/script>/.exec(html)
          if (!module) throw new Error('The fixture has no module entry')
          const route =
            '/nested/prefix/' + (kind === 'module' ? module[1]!.replace(/^\.\//, '') : '')
          redirectServer = ((listener: RequestListener) =>
            createServer((request, response) => {
              if (request.url === route) {
                redirected = true
                response.writeHead(302, { Location: destination }).end()
              } else listener(request, response)
            })) as typeof createServer
        })

        it('should reject the HTTP redirect destination even though it returns 200', async () => {
          await expect(
            checkBuild({ dist, prefix: '/nested/prefix/', createServer: redirectServer })
          ).rejects.toThrow(
            (kind === 'module' ? 'a request' : 'the document') +
              ' left the expected origin or prefix: ' +
              destination
          )
          expect(redirected).toBe(true)
          expect(requests).toContain(kind === 'module' ? '/escape.js' : '/landed.html')
        }, 120_000)
      }
    )

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

      // The check reads the page only once the top frame stops loading, so it sees the document the redirect lands on,
      // which the second origin has served by then.
      const escaped = `${origin}/landed.html`.replaceAll('.', '\\.')
      await expect(checkBuild({ dist: copy, prefix: '/nested/prefix/' })).rejects.toThrow(
        new RegExp(`the document left the expected origin or prefix: ${escaped}$`)
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

  describe('and history escapes after the final audit render is measured', () => {
    let escaped: string
    let auditLocation: string
    let auditBox: { width: number; height: number } | undefined
    let open: typeof openSession

    beforeEach(() => {
      escaped = ''
      auditLocation = ''
      auditBox = undefined
      open = async (...args) => {
        const session = await openSession(...args)
        const send = session.send
        session.send = async (method, params) => {
          const result = await send(method, params)
          if (
            method === 'Runtime.evaluate' &&
            String(params?.expression).includes(
              "document.querySelector('#audit-render')?.firstElementChild"
            )
          ) {
            const box = (result as { result?: { value?: { width: number; height: number } } })
              .result?.value
            if (box && box.width > 0 && box.height > 0) {
              auditBox = box
              const moved = (await send('Runtime.evaluate', {
                expression:
                  '(() => { const before = location.href; history.replaceState(null, "", "/outside/final-audit.html"); return { before, after: location.href }; })()',
                returnByValue: true
              })) as { result: { value: { before: string; after: string } } }
              auditLocation = moved.result.value.before
              escaped = moved.result.value.after
            }
          }
          return result
        }
        return session
      }
    })

    it('should reject the observed escape after reaching the actual gallery audit step', async () => {
      const proof = checkBuild({ dist, prefix: '/nested/prefix/', open })
      await proof.catch(() => undefined)
      expect(auditLocation).toMatch(/\/nested\/prefix\/dcl-ui-review\/.*[?&]audit=1(?:&|$)/)
      expect(auditBox?.width).toBeGreaterThan(0)
      expect(auditBox?.height).toBeGreaterThan(0)
      expect(escaped).toMatch(/^http:\/\/127\.0\.0\.1:\d+\/outside\/final-audit\.html$/)
      await expect(proof).rejects.toThrow(
        'the document left the expected origin or prefix: ' + escaped
      )
    }, 120_000)
  })

  describe('and a navigation replaces the page while a poll is pending', () => {
    // Rewrites the first poll for the component heading: `poll` is what that evaluation runs instead.
    const interrupting = (poll: string) => {
      const seen = { rewritten: false, rejected: '', polls: 0 }
      const open: typeof openSession = async (...args) => {
        const session = await openSession(...args)
        const send = session.send
        session.send = async (method, params) => {
          const heading =
            method === 'Runtime.evaluate' &&
            String(params?.expression).includes("document.querySelector('main h1')")
          if (heading) seen.polls++
          if (heading && !seen.rewritten) {
            seen.rewritten = true
            try {
              return await send(method, { ...params, expression: poll })
            } catch (error) {
              seen.rejected = String(error)
              throw error
            }
          }
          return send(method, params)
        }
        return session
      }
      return { open, seen }
    }

    it('should read the poll again in the document that replaced it', async () => {
      // The evaluation never settles by itself, so it is in flight when the reload commits.
      const { open, seen } = interrupting('(location.reload(), new Promise(() => {}))')

      await expect(checkBuild({ dist, open })).resolves.toMatchObject({ entry: '/index.html' })
      expect(seen.rewritten).toBe(true)
      // Chrome names the lost document or the lost context, depending on how it replaced the page.
      expect(seen.rejected).toMatch(
        /Inspected target navigated or closed|Execution context was destroyed|Cannot find context/
      )
    }, 120_000)

    it('should not keep a value read while a navigation started, and read the new document instead', async () => {
      // The poll answers true, but the reload it starts is reported before that answer arrives.
      const { open, seen } = interrupting('(location.reload(), true)')

      await expect(checkBuild({ dist, open })).resolves.toMatchObject({ entry: '/index.html' })
      expect(seen.rewritten).toBe(true)
      expect(seen.rejected).toBe('')
      expect(seen.polls).toBeGreaterThan(1)
    }, 120_000)

    it('should fail on a lost context that no navigation explains', async () => {
      const open: typeof openSession = async (...args) => {
        const session = await openSession(...args)
        const send = session.send
        session.send = async (method, params) => {
          if (
            method === 'Runtime.evaluate' &&
            String(params?.expression).includes("document.querySelector('main h1')")
          )
            throw Object.assign(
              new Error('{"code":-32000,"message":"Inspected target navigated or closed"}'),
              { cdp: { code: -32000, message: 'Inspected target navigated or closed' } }
            )
          return send(method, params)
        }
        return session
      }

      await expect(checkBuild({ dist, open })).rejects.toThrow(
        'Inspected target navigated or closed'
      )
    }, 120_000)

    it('should time out on a page that never finishes loading instead of reading it', async () => {
      let reads = 0
      const loading = async () => ({
        requests: [],
        requestFailures: [],
        runtimeErrors: [],
        consoleErrors: [],
        navigations: 0,
        loading: true,
        send: async () => ({}),
        navigate: async () => ({}),
        whenLoaded: async () => {},
        evaluate: async () => {
          reads++
          return 'http://127.0.0.1/'
        },
        close: async () => {}
      })

      await expect(checkBuild({ dist, browser: 'unused', open: loading })).rejects.toThrow(
        /^Timed out waiting for the page at /
      )
      expect(reads).toBe(0)
    }, 60_000)
  })

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
        navigations: 0,
        loading: false,
        navigate: async (url: string) => {
          origin ||= new URL(url).origin
          return {}
        },
        whenLoaded: async () => {},
        send: async () => ({}),
        evaluate: async (expression: string) => evaluate(expression),
        close: async () => {
          if (!closes) throw new Error('browser would not close')
        }
      })
    // Answers every check the way a working gallery does.
    const passing = (expression: string) => {
      const page = `${origin}/?component=Button`
      if (
        expression.includes('return location.href }') ||
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
