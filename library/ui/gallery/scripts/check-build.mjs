#!/usr/bin/env node
// gallery check-build: functional proof that the built runtime gallery works where it is served, beside the generated
// Review. It serves <dist> (the combined output: the generated Review at its root, the gallery in gallery/) from a
// local server under a URL prefix (default "/"), drives Chromium and fails unless:
//   1. the gallery entry (<prefix>gallery/) lists runtime and generated pages, labelled by source;
//   2. every runtime page draws every example in its iframe canvas with a nonzero box and no example error;
//   3. the canvas is a real viewport: NavBar shows its menu button at 375px and hides it at 1280px;
//   4. every runtime foundations page renders;
//   5. the generated route opens the generated Review's page for a component inside the gallery;
// with every request answered 2xx from under the prefix, and no uncaught page error or console error along the way.
// It runs the built files it is given, so only run it on an unprivileged build.
import { readFile, stat } from 'node:fs/promises'
import { createServer } from 'node:http'
import path from 'node:path'
import process from 'node:process'
import { setTimeout as delay } from 'node:timers/promises'
import { fileURLToPath } from 'node:url'
import {
  NavigationInterrupted,
  findBrowser,
  openSession
} from '../../figma/review/scripts/capture-browser.mjs'

const TYPES = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.json': 'application/json',
  '.svg': 'image/svg+xml',
  '.woff2': 'font/woff2',
  '.woff': 'font/woff',
  '.png': 'image/png'
}
const TIMEOUT = 20_000
const BROWSERS = [
  process.env.BROWSER_PATH,
  process.env.CHROME_PATH,
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium',
  '/usr/bin/chromium-browser'
]

function normalizePrefix(prefix) {
  if (!prefix.startsWith('/') || prefix.includes('..') || prefix.includes('\\'))
    throw new Error('--prefix must be an absolute URL path such as /nested/prefix/')
  return prefix.endsWith('/') ? prefix : `${prefix}/`
}

/** Serves `dist` under `prefix` on the loopback interface and records every request with its status. */
async function serve(dist, prefix) {
  const requests = []
  const server = createServer((request, response) => {
    void (async () => {
      const url = new URL(request.url ?? '/', 'http://127.0.0.1')
      const record = (status) =>
        url.pathname !== '/favicon.ico' && requests.push({ path: url.pathname, status })
      if (!url.pathname.startsWith(prefix)) {
        record(404)
        return void response.writeHead(404).end()
      }
      let file
      try {
        file = path.join(dist, decodeURIComponent(url.pathname.slice(prefix.length)))
      } catch {
        record(400)
        return void response.writeHead(400).end()
      }
      const relative = path.relative(dist, file)
      if (relative === '..' || relative.startsWith('..' + path.sep) || path.isAbsolute(relative)) {
        record(403)
        return void response.writeHead(403).end()
      }
      try {
        if ((await stat(file)).isDirectory()) file = path.join(file, 'index.html')
        const body = await readFile(file)
        record(200)
        response.writeHead(200, {
          'content-type': TYPES[path.extname(file)] ?? 'application/octet-stream'
        })
        response.end(body)
      } catch {
        record(404)
        response.writeHead(404).end()
      }
    })()
  })
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve))
  return {
    origin: `http://127.0.0.1:${server.address().port}`,
    requests,
    close: () =>
      new Promise((resolve, reject) => {
        server.close((error) => (error ? reject(error) : resolve()))
        server.closeAllConnections()
      })
  }
}

// Polls a page expression until it returns a truthy value in a top document that has finished loading. A poll that
// a navigation interrupted is read again in the document that replaced it.
async function waitFor(session, expression, what) {
  const started = Date.now()
  for (;;) {
    if (!session.loading) {
      let value = null
      try {
        value = await session.evaluate(
          `(() => { try { return ${expression} } catch { return null } })()`
        )
      } catch (error) {
        if (!(error instanceof NavigationInterrupted)) throw error
      }
      if (value && !session.loading) return value
    }
    if (Date.now() - started > TIMEOUT) throw new Error(`Timed out waiting for ${what}`)
    await delay(100)
  }
}

// The canvas document inside the gallery page, once it has loaded.
const CANVAS = `document.querySelector('iframe')?.contentDocument`
// Every example of a runtime page: its id, whether its stage draws a nonzero box, and any example error.
const EXAMPLES = `(() => {
  const doc = ${CANVAS}
  if (!doc || doc.readyState !== 'complete' || !doc.querySelector('[data-example]')) return null
  return [...doc.querySelectorAll('[data-example]')].map((section) => ({
    id: section.dataset.example,
    drawn: [...section.querySelectorAll('[data-stage] *')].some((node) => {
      const box = node.getBoundingClientRect()
      return box.width > 0 && box.height > 0
    }),
    error: section.querySelector('[data-example-error]')?.textContent ?? null
  }))
})()`

/** Runs the checks. Resolves with a summary; rejects naming the first failed check. */
async function checkGallery({ dist, prefix = '/', browser } = {}) {
  const base = normalizePrefix(prefix)
  const server = await serve(path.resolve(dist), base)
  const gallery = `${server.origin}${base}gallery/`
  let session
  let summary
  let failure
  try {
    session = await openSession(browser ?? (await findBrowser(BROWSERS)))
    const checkRequests = () => {
      for (const request of session.requests) {
        const url = new URL(request.url)
        if (url.protocol === 'data:' && request.type !== 'Document') continue
        if (url.origin === server.origin && url.pathname === '/favicon.ico') continue
        if (url.origin !== server.origin || !url.pathname.startsWith(base))
          throw new Error(`a request left the expected origin or prefix: ${request.url}`)
      }
      const failed = session.requestFailures.find(
        (request) => !request.url?.endsWith('/favicon.ico')
      )
      if (failed) throw new Error(`browser request failed: ${failed.url} (${failed.error})`)
    }
    const open = async (query) => {
      await session.navigate(`${gallery}${query}`, TIMEOUT)
      await waitFor(session, `document.readyState === 'complete'`, `the gallery at ${query}`)
    }

    await open('')
    const listed = await waitFor(
      session,
      `(() => {
        const names = (source) => [...document.querySelectorAll('nav a[data-source="' + source + '"]')]
          .map((a) => new URL(a.href).searchParams.get('component')).filter(Boolean)
        const runtime = names('runtime'), generated = names('generated')
        return runtime.length && generated.length ? { runtime, generated } : null
      })()`,
      'runtime and generated navigation'
    )

    const examples = []
    for (const name of listed.runtime) {
      await open(`?component=${encodeURIComponent(name)}&width=1280`)
      const drawn = await waitFor(
        session,
        `${CANVAS}?.querySelector('main')?.dataset.component === ${JSON.stringify(name)} && ${EXAMPLES}`,
        `the ${name} canvas`
      )
      for (const example of drawn) {
        if (example.error) throw new Error(`${name} ${example.id} failed: ${example.error}`)
        if (!example.drawn) throw new Error(`${name} ${example.id} draws nothing`)
      }
      examples.push(...drawn.map((example) => `${name}/${example.id}`))
    }

    const menuShown = async (width) => {
      await open(`?component=NavBar&width=${width}`)
      return waitFor(
        session,
        `(() => {
          const doc = ${CANVAS}
          const frame = document.querySelector('iframe')
          if (!doc || doc.readyState !== 'complete' || frame.clientWidth !== ${width}) return null
          const button = doc.querySelector('[data-example="signed-out"] button[aria-label="Open menu"]')
          if (!button) return null
          const box = button.getBoundingClientRect()
          return { shown: box.width > 0 && box.height > 0 && getComputedStyle(button).visibility !== 'hidden' }
        })()`,
        `NavBar at ${width}px`
      )
    }
    if (!(await menuShown(375)).shown)
      throw new Error('NavBar shows no menu button at 375px: the canvas is not a viewport')
    if ((await menuShown(1280)).shown)
      throw new Error('NavBar shows the menu button at 1280px: the canvas is not a viewport')

    const foundations = await session.evaluate(
      `[...document.querySelectorAll('nav a[data-source="runtime"]')].map((a) => new URL(a.href).searchParams.get('foundations')).filter(Boolean)`
    )
    for (const name of foundations) {
      await open(`?foundations=${encodeURIComponent(name)}`)
      await waitFor(
        session,
        `(${CANVAS}?.querySelector('[data-foundations=${JSON.stringify(name)}]')?.textContent ?? '').length > 0`,
        `the ${name} foundations page`
      )
    }

    const generated = listed.generated[0]
    await open(`?source=generated&component=${encodeURIComponent(generated)}`)
    await waitFor(
      session,
      `${CANVAS}?.querySelector('main h1')?.textContent === ${JSON.stringify(generated)}`,
      `the generated ${generated} page inside the gallery`
    )

    checkRequests()
    const failed = server.requests.filter((request) => request.status >= 400)
    if (failed.length)
      throw new Error(
        `${failed.length} requests failed, first ${failed[0].status} ${failed[0].path}`
      )
    if (session.runtimeErrors.length) throw new Error(`page error: ${session.runtimeErrors[0]}`)
    if (session.consoleErrors.length) throw new Error(`console error: ${session.consoleErrors[0]}`)
    summary = {
      entry: `${base}gallery/`,
      runtimePages: listed.runtime.length,
      examples: examples.length,
      foundations: foundations.length,
      generatedPages: listed.generated.length,
      generated,
      requests: server.requests.length
    }
  } catch (error) {
    failure = { error }
  }
  for (const step of [() => session?.close(), () => server.close()]) {
    try {
      await step()
    } catch (error) {
      failure ??= { error }
    }
  }
  if (failure) throw failure.error
  return summary
}

/** Command-line entry point. Returns the exit code. */
async function main(argv) {
  const [dist, ...rest] = argv
  let prefix = '/'
  if (rest[0] === '--prefix' && rest.length === 2) prefix = rest[1]
  else if (rest.length || !dist) {
    process.stderr.write('Usage: check-build <dist> [--prefix <path>]\n')
    return 2
  }
  try {
    process.stdout.write(`${JSON.stringify(await checkGallery({ dist, prefix }), null, 2)}\n`)
    return 0
  } catch (error) {
    process.stderr.write(
      `gallery check-build: ${error instanceof Error ? error.message : String(error)}\n`
    )
    return 1
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = await main(process.argv.slice(2))
}

export { checkGallery, main }
