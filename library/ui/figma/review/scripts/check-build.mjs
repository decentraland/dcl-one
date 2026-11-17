#!/usr/bin/env node
// check-build: functional proof that a built Review gallery works where it is served. It serves <review-dist> from
// a local server under a URL prefix (default "/"), drives Chromium through the gallery and fails unless:
//   1. the root entry (<prefix>index.html) and every request it makes answer 2xx or 3xx, from under the prefix,
//      and every document the page ends on, after any redirect or history change, is under the prefix too;
//   2. the navigation lists components from the built catalog;
//   3. following a component link opens that component's page;
//   4. choosing another variant through the variant selector opens that variant;
//   5. the audit render of a catalog component draws a root element with a nonzero box;
// with no uncaught page error and no console error along the way. It reads no evidence file: the gallery
// publishes no Figma imagery. It runs the built files it is given, so only run it on an unprivileged build.
import { readFile, stat } from 'node:fs/promises'
import { createServer as createHttpServer } from 'node:http'
import path from 'node:path'
import process from 'node:process'
import { setTimeout as delay } from 'node:timers/promises'
import { fileURLToPath } from 'node:url'
import { NavigationInterrupted, findBrowser, openSession } from './capture-browser.mjs'

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

/**
 * Serves `dist` under `prefix` on the loopback interface; records every request with its status. `createServer`
 * builds the HTTP server.
 */
async function serve(dist, prefix, createServer) {
  const requests = []
  const server = createServer((request, response) => {
    void (async () => {
      const url = new URL(request.url ?? '/', 'http://127.0.0.1')
      // The browser asks for /favicon.ico on its own; the page never references it.
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
  const { port } = server.address()
  return {
    origin: `http://127.0.0.1:${port}`,
    requests,
    // Drop open keep-alive sockets so they cannot hold the process, and surface a failed close.
    close: () =>
      new Promise((resolve, reject) => {
        server.close((error) => (error ? reject(error) : resolve()))
        server.closeAllConnections()
      })
  }
}

// Polls a page expression until it returns a truthy value in a document that has finished loading. A value is kept
// only when the top frame was idle and started no navigation while it was read, since it may describe the outgoing
// document. A poll that a navigation interrupted is read again in the document that replaced it; any other error
// fails, and a page that never settles times out.
async function waitFor(session, expression, what) {
  const started = Date.now()
  for (;;) {
    if (!session.loading) {
      const before = session.navigations
      let value = null
      try {
        value = await session.evaluate(
          `(() => { try { return ${expression} } catch { return null } })()`
        )
      } catch (error) {
        if (!(error instanceof NavigationInterrupted)) throw error
      }
      if (value && !session.loading && session.navigations === before) return value
    }
    if (Date.now() - started > TIMEOUT) throw new Error(`Timed out waiting for ${what}`)
    await delay(100)
  }
}

async function navigate(session, url, check) {
  await session.navigate(url, TIMEOUT)
  const loaded = await waitFor(
    session,
    `document.readyState === 'complete' && location.href`,
    `the page at ${url}`
  )
  check(loaded)
}

const componentLinks = `[...document.querySelectorAll('a[href*="component="]')].map((a) => ({ href: a.href, name: new URL(a.href).searchParams.get('component') })).filter((l) => l.name)`

/**
 * Runs the five checks. Resolves with a summary; rejects naming the first failed check. `browser` overrides the
 * Chromium executable, `open` the function that starts its session and `createServer` the one that builds the
 * HTTP server.
 */
async function checkBuild({
  dist,
  prefix = '/',
  browser,
  open = openSession,
  createServer = createHttpServer
} = {}) {
  const root = path.resolve(dist)
  const base = normalizePrefix(prefix)
  const server = await serve(root, base, createServer)
  let session
  let summary
  let failure
  try {
    session = await open(browser ?? (await findBrowser(BROWSERS)))

    // Keep the existing exception only for Chromium's automatic favicon lookup, not page-initiated assets.
    const automaticFavicon = (request) => {
      const url = new URL(request.url)
      return (
        request.type === 'Other' &&
        request.initiator === 'other' &&
        url.origin === server.origin &&
        url.pathname === '/favicon.ico'
      )
    }
    const checkRequests = () => {
      for (const request of session.requests) {
        if (automaticFavicon(request)) continue
        const url = new URL(request.url)
        // Inline resources make no network request. A document must still stay under the served prefix.
        if (
          request.type !== 'Document' &&
          (url.protocol === 'data:' || (url.protocol === 'blob:' && url.origin === server.origin))
        )
          continue
        if (url.origin !== server.origin || !url.pathname.startsWith(base))
          throw new Error(`a request left the expected origin or prefix: ${request.url}`)
      }
      const failed = session.requestFailures.find(
        (request) => !request.url || !automaticFavicon(request)
      )
      if (failed)
        throw new Error(
          `browser requests failed, first: ${failed.url ?? 'unknown URL'} (${failed.error})`
        )
    }
    // The document the page holds now, after any redirect or history change, must be served under the prefix too.
    const checkDocument = (href) => {
      // Chromium shows its own error page when a document answers an error or fails to load.
      if (href.startsWith('chrome-error:'))
        throw new Error(`browser requests failed, first: the document did not load (${href})`)
      const url = new URL(href)
      if (url.origin !== server.origin || !url.pathname.startsWith(base))
        throw new Error(`the document left the expected origin or prefix: ${href}`)
    }
    const checkPage = (href) => {
      checkDocument(href)
      checkRequests()
    }

    await navigate(session, `${server.origin}${base}`, checkPage)
    const links = await waitFor(
      session,
      `(${componentLinks}).length ? ${componentLinks} : null`,
      'catalog navigation'
    )
    const first = links[0]

    await navigate(session, first.href, checkPage)
    await waitFor(
      session,
      `document.querySelector('main h1')?.textContent === ${JSON.stringify(first.name)}`,
      `the ${first.name} page`
    )

    const variantSwitch = await session.evaluate(`(() => {
      const select = [...document.querySelectorAll('select[name^="variant-"]')].find((s) => s.options.length > 1)
      if (!select) return null
      const next = [...select.options].find((o) => o.value !== select.value)
      select.value = next.value
      select.dispatchEvent(new Event('change', { bubbles: true }))
      return { property: select.name.slice('variant-'.length), value: next.value }
    })()`)
    if (!variantSwitch)
      throw new Error('The first component offers no second variant to navigate to')
    const variant = await waitFor(
      session,
      `new URL(location.href).searchParams.has('variant') && document.readyState === 'complete' && document.querySelector('select[name=${JSON.stringify(`variant-${variantSwitch.property}`)}]')?.value === ${JSON.stringify(variantSwitch.value)} && new URL(location.href).searchParams.get('variant')`,
      'variant navigation'
    )
    checkPage(await waitFor(session, 'location.href', 'the current document'))

    const audit = new URL(first.href)
    audit.searchParams.set('audit', '1')
    audit.searchParams.set('variant', '0')
    await navigate(session, audit.href, checkPage)
    const box = await waitFor(
      session,
      `(() => { const r = document.querySelector('#audit-render')?.firstElementChild?.getBoundingClientRect(); return r && r.width > 0 && r.height > 0 ? { width: r.width, height: r.height } : null })()`,
      'a non-empty audit render'
    )

    const failed = server.requests.filter((request) => request.status >= 400)
    if (failed.length)
      throw new Error(
        `${failed.length} requests failed, first ${failed[0].status} ${failed[0].path}`
      )
    checkPage(await waitFor(session, 'location.href', 'the current document'))
    if (session.runtimeErrors.length) throw new Error(`page error: ${session.runtimeErrors[0]}`)
    if (session.consoleErrors.length) throw new Error(`console error: ${session.consoleErrors[0]}`)
    summary = {
      entry: `${base}index.html`,
      components: links.length,
      component: first.name,
      variant: Number(variant),
      audit: box,
      requests: server.requests.length
    }
  } catch (error) {
    failure = { error }
  }
  // Every teardown step runs, so a session that fails to close still releases the server; the first error, from
  // the check or else from teardown, is what the caller sees.
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
  else if (rest.length) {
    process.stderr.write('Usage: check-build <review-dist> [--prefix <path>]\n')
    return 2
  }
  if (!dist) {
    process.stderr.write('Usage: check-build <review-dist> [--prefix <path>]\n')
    return 2
  }
  try {
    process.stdout.write(`${JSON.stringify(await checkBuild({ dist, prefix }), null, 2)}\n`)
    return 0
  } catch (error) {
    process.stderr.write(`check-build: ${error instanceof Error ? error.message : String(error)}\n`)
    return 1
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = await main(process.argv.slice(2))
}

export { checkBuild, main }
