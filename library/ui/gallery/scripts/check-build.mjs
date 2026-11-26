#!/usr/bin/env node
// gallery check-build: functional proof that the built runtime gallery works where it is served, beside the generated
// Review. It serves <dist> (the combined output: the generated Review at its root, the gallery in gallery/) from a
// local server under a URL prefix (default "/"), drives Chromium and fails unless:
//   1. the gallery entry (<prefix>gallery/) lists runtime and generated pages, labelled by source;
//   2. every runtime page draws every example in its iframe canvas with a nonzero box and no example error, and at
//      1280px and 375px keeps its layout (LAYOUT): the page fits the viewport, every picture loads and paints inside
//      its own box, and no in-flow text covers other text; so do the foundations pages and the HUD Showcase; at 375px
//      the gallery page around it fits a 375px window too; the components Showcase, the first runtime page listed,
//      passes the same checks and draws every other runtime page, in the sidebar's order, each with its examples
//      drawn and none failed, a page with a canvas of its own in a canvas of the chosen width;
//   3. the canvas is a real viewport: NavBar shows its menu button at 375px and hides it at 1280px;
//   4. every runtime foundations page renders;
//   5. the HUD section lists the Explorer HUD's Showcase first, then its pages; the Showcase page draws every page
//      in its canvas, which is a HUD window (the height its aspect ratio and minimum give it, at the --ui-scale
//      hudScale.ts computes for that window), the HudScale section shows that scale and follows a width change, and
//      a component opens the section that renders it;
//   6. the generated route opens the generated Review's page for a component inside the gallery;
// with every request answered 2xx from under the prefix, and no uncaught page error or console error along the way.
// With --hosted, <dist> is the hosted pull request preview (`vite build --mode hosted`): the gallery alone at its root,
// opened as <prefix>index.html the way the bucket serves it, listing no generated page; check 6 becomes a Figma
// reference link that answers with a note and loads nothing from the generated Review.
// With --csp, every response carries that Content-Security-Policy, as the preview host sends it, and a violation the
// browser reports fails the check.
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
import { expectedHudScale, expectedWindowHeight, readHudScale } from './hud-window.mjs'

const TYPES = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.json': 'application/json',
  '.svg': 'image/svg+xml',
  '.woff2': 'font/woff2',
  '.woff': 'font/woff',
  '.png': 'image/png',
  '.webp': 'image/webp'
}
const TIMEOUT = 20_000
// Where the browser posts Content-Security-Policy violations; outside every prefix the check serves.
const CSP_REPORTS = '/.csp-reports'
// A report is posted after the load that caused it, so the last page gets this long to send its own.
const CSP_REPORT_WAIT = 500
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
 * Serves `dist` under `prefix` on the loopback interface and records every request with its status. With `csp`, every
 * file carries that policy and the violations the browser reports are recorded too.
 */
async function serve(dist, prefix, csp) {
  const requests = []
  const violations = []
  const headers = csp ? { 'content-security-policy': `${csp}; report-uri ${CSP_REPORTS}` } : {}
  const server = createServer((request, response) => {
    void (async () => {
      const url = new URL(request.url ?? '/', 'http://127.0.0.1')
      if (csp && request.method === 'POST' && url.pathname === CSP_REPORTS) {
        let body = ''
        for await (const chunk of request) body += chunk
        let report = {}
        try {
          report = JSON.parse(body)['csp-report'] ?? {}
        } catch {
          // An unreadable report is still a violation.
        }
        const directive = report['effective-directive'] ?? 'a directive'
        const blocked = report['blocked-uri'] || 'inline content'
        violations.push(`${directive} blocked ${blocked} in ${report['document-uri'] ?? 'a page'}`)
        return void response.writeHead(204).end()
      }
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
          ...headers,
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
    violations,
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

// The gallery's name for the components Showcase (COMPONENT_OVERVIEW in src/componentPages.ts).
const COMPONENT_OVERVIEW = 'Showcase'
// The pages the components Showcase draws, in order, or null while one is still loading. A page positioned against
// the viewport has a canvas of its own, loaded as it nears the window: the check loads each now, waits for it and
// reads its document, with the width of that canvas. A page is drawn when it has examples and each one draws a nonzero
// box; `error` is its first example error.
const SHOWCASE_PAGES = `(() => {
  const pages = []
  for (const page of ${CANVAS}.querySelectorAll('[data-showcase-page]')) {
    const frame = page.querySelector('iframe')
    let root = page
    if (frame) {
      if (frame.loading === 'lazy') frame.loading = 'eager'
      root = frame.contentDocument
      if (!root || root.readyState !== 'complete' || !root.querySelector('#root')?.children.length) return null
    }
    const examples = [...root.querySelectorAll('[data-example]')]
    pages.push({
      name: page.dataset.showcasePage,
      width: frame ? frame.clientWidth : null,
      error: root.querySelector('[data-example-error]')?.textContent ?? null,
      drawn: !page.querySelector('[data-showcase-empty]') && examples.length > 0 &&
        examples.every((section) => [...section.querySelectorAll('[data-stage] *')].some((node) => {
          const box = node.getBoundingClientRect()
          return box.width > 0 && box.height > 0
        }))
    })
  }
  return pages
})()`

// How far, in CSS pixels, a picture the component positions may overhang its slot.
const POSITIONED_OVERHANG = 4

// What breaks an example's layout in a canvas document, as a list of problems (empty when none), or null while a
// picture or a web font is still loading. An example must fit the viewport (no page wider than it), every picture
// must load and paint inside its slot (a cover larger than its slot spills over the text beside it; a picture the
// component positions may overhang it by POSITIONED_OVERHANG), and no line of in-flow text may cover another one.
// Text the component positions itself (a badge, a toast, an overlay), hides for assistive technology or clips by
// design (ellipsis, line clamp, scroller) is placed on purpose.
const LAYOUT = `(() => {
  const doc = ${CANVAS}
  const view = doc?.defaultView
  if (!doc || doc.readyState !== 'complete') return null
  // A lazy picture below the fold would never load in the headless window and escape the check: every picture loads
  // now, and the check waits (within waitFor's bound) until each has loaded or failed.
  for (const image of doc.images) if (image.loading === 'lazy') image.loading = 'eager'
  if ([...doc.images].some((image) => image.getAttribute('src') && !image.complete)) return null
  // Layout first: it is what starts a web font's load. A measure taken in the fallback font passes a page that
  // overlaps once the font arrives, so the page is measured only when no font is loading, before and after.
  void doc.body.offsetHeight
  if (doc.fonts.status !== 'loaded') return null
  const problems = []
  const where = (node) => (node.closest('[data-example]')?.dataset.example ?? '-')
  const name = (node) => node.tagName.toLowerCase() + (typeof node.className === 'string' && node.className ? '.' + node.className.split(' ')[0] : '')
  if (doc.documentElement.scrollWidth > view.innerWidth + 1)
    problems.push('the page is ' + doc.documentElement.scrollWidth + 'px wide in a ' + view.innerWidth + 'px viewport')
  for (const image of doc.images)
    if (image.complete && image.naturalWidth === 0) problems.push(where(image) + ': the picture ' + image.src.slice(0, 80) + ' does not load')
  const shown = (node) => {
    for (let at = node; at && at !== doc.body; at = at.parentElement) {
      const style = view.getComputedStyle(at)
      if (style.display === 'none' || style.visibility === 'hidden' || Number(style.opacity) === 0) return false
      if (style.clipPath.includes('inset(50%)') || (style.clip && style.clip !== 'auto')) return false
    }
    return true
  }
  // The part of a box that paints: only the ancestors that clip it cut it, each on the axes it clips. Overflow other
  // than visible clips its own axis (clip, hidden, auto, scroll; a visible beside a non-clip value computes to auto),
  // paint containment clips both, and a positioned box escapes every ancestor up to its containing block.
  const containsPaint = (style) => /paint|strict|content/.test(style.contain)
  const contains = (style, position) =>
    position === 'fixed'
      ? style.transform !== 'none' || style.filter !== 'none' || style.perspective !== 'none' || /paint|layout|strict|content/.test(style.contain)
      : style.position !== 'static' || style.transform !== 'none' || style.filter !== 'none' || /paint|layout|strict|content/.test(style.contain)
  const painted = (start, box, position) => {
    let out = { left: box.left, right: box.right, top: box.top, bottom: box.bottom }
    for (let at = start; at && at !== doc.documentElement; at = at.parentElement) {
      const style = view.getComputedStyle(at)
      if (position === 'absolute' || position === 'fixed') {
        if (!contains(style, position)) continue
      }
      position = style.position
      const clip = at.getBoundingClientRect()
      const paint = containsPaint(style)
      if (paint || style.overflowX !== 'visible') out = { ...out, left: Math.max(out.left, clip.left), right: Math.min(out.right, clip.right) }
      if (paint || style.overflowY !== 'visible') out = { ...out, top: Math.max(out.top, clip.top), bottom: Math.min(out.bottom, clip.bottom) }
    }
    return { ...out, width: Math.max(0, out.right - out.left), height: Math.max(0, out.bottom - out.top) }
  }
  const overlay = (node, stage) => {
    for (let at = node; at && at !== stage; at = at.parentElement)
      if (['absolute', 'fixed', 'sticky'].includes(view.getComputedStyle(at).position)) return true
    return false
  }
  for (const stage of doc.querySelectorAll('[data-stage]')) {
    for (const media of stage.querySelectorAll('img, video')) {
      const holder = media.parentElement
      if (!shown(media) || holder === stage) continue
      const position = view.getComputedStyle(media).position
      const box = painted(media.parentElement, media.getBoundingClientRect(), position)
      const slot = holder.getBoundingClientRect()
      // A picture the component positions may overhang its slot by a few pixels: a corner badge set over the edge of
      // its button (the HUD IconButton's corner art, 3px). A measured bound, not a list of components, so a new badge
      // needs no entry and a positioned cover that escapes its slot still fails.
      const allowed = ['absolute', 'fixed'].includes(position) ? ${POSITIONED_OVERHANG} : 1
      const overhang = Math.max(slot.left - box.left, box.right - slot.right, slot.top - box.top, box.bottom - slot.bottom)
      if (box.width > 1 && box.height > 1 && overhang > allowed)
        problems.push(where(media) + ': a picture paints ' + Math.round(box.width) + 'x' + Math.round(box.height) + ' outside its ' + Math.round(slot.width) + 'x' + Math.round(slot.height) + ' ' + name(holder))
    }
    const lines = []
    const walker = doc.createTreeWalker(stage, NodeFilter.SHOW_TEXT)
    for (let text = walker.nextNode(); text; text = walker.nextNode()) {
      const owner = text.parentElement
      if (!text.textContent.trim() || !shown(owner) || overlay(owner, stage)) continue
      const range = doc.createRange()
      range.selectNodeContents(text)
      for (const line of range.getClientRects()) {
        const box = painted(owner, line, 'static')
        if (box.width > 1 && box.height > 1) lines.push({ owner, box, text: text.textContent.trim().slice(0, 30) })
      }
    }
    for (let i = 0; i < lines.length; i += 1)
      for (let j = i + 1; j < lines.length; j += 1) {
        const a = lines[i], b = lines[j]
        if (a.owner === b.owner || a.owner.contains(b.owner) || b.owner.contains(a.owner)) continue
        const shared = Math.max(0, Math.min(a.box.right, b.box.right) - Math.max(a.box.left, b.box.left)) *
          Math.max(0, Math.min(a.box.bottom, b.box.bottom) - Math.max(a.box.top, b.box.top))
        if (shared > 0.2 * Math.min(a.box.width * a.box.height, b.box.width * b.box.height))
          problems.push(where(a.owner) + ': "' + a.text + '" overlaps "' + b.text + '"')
      }
  }
  if (doc.fonts.status !== 'loaded') return null
  return [...new Set(problems)]
})()`

// The scroll offsets that bring every part of the canvas into view, one viewport apart. LAYOUT measures what paints,
// and a canvas that scrolls inside its own box (the HUD window) clips whatever lies beyond it, so each offset is
// measured in turn. A canvas that does not scroll inside itself has the one offset 0.
const SCROLL_STEPS = `(() => {
  const doc = ${CANVAS}
  const view = doc?.defaultView
  const first = doc?.querySelector('[data-stage]')
  if (!first) return [0]
  for (let at = first.parentElement; at && at !== doc.documentElement && at !== doc.body; at = at.parentElement) {
    if (!/auto|scroll/.test(view.getComputedStyle(at).overflowY) || at.scrollHeight <= at.clientHeight) continue
    at.setAttribute('data-layout-scroller', '')
    const steps = []
    for (let top = 0; top < at.scrollHeight - at.clientHeight; top += at.clientHeight) steps.push(top)
    steps.push(at.scrollHeight - at.clientHeight)
    return steps
  }
  return [0]
})()`

/**
 * Waits for the canvas's pictures and fonts, then throws naming every layout problem LAYOUT finds on a page, at each
 * scroll offset of a canvas that scrolls inside itself.
 */
async function checkLayout(session, page) {
  const problems = new Set(await waitFor(session, LAYOUT, `the pictures and fonts of ${page}`))
  const steps = await session.evaluate(SCROLL_STEPS)
  for (const top of steps.slice(1)) {
    await session.evaluate(
      `new Promise((resolve) => { const doc = ${CANVAS}; doc.querySelector('[data-layout-scroller]').scrollTop = ${top}; doc.defaultView.requestAnimationFrame(() => doc.defaultView.requestAnimationFrame(resolve)) })`
    )
    for (const problem of await waitFor(
      session,
      LAYOUT,
      `the pictures and fonts of ${page} at ${top}px`
    ))
      problems.add(`${problem} (scrolled ${top}px)`)
  }
  if (problems.size) throw new Error(`${page} breaks its layout: ${[...problems].join('; ')}`)
}

/** Runs the checks. Resolves with a summary; rejects naming the first failed check. */
async function checkGallery({ dist, prefix = '/', hosted = false, csp, browser } = {}) {
  const base = normalizePrefix(prefix)
  const server = await serve(path.resolve(dist), base, csp)
  const gallery = `${server.origin}${base}${hosted ? 'index.html' : 'gallery/'}`
  let session
  let summary
  let failure
  const violation = () =>
    server.violations.length
      ? new Error(`Content-Security-Policy violation: ${server.violations[0]}`)
      : undefined
  try {
    session = await openSession(browser ?? (await findBrowser(BROWSERS)))
    const checkRequests = () => {
      const blocked = violation()
      if (blocked) throw blocked
      for (const request of session.requests) {
        const url = new URL(request.url)
        if (url.protocol === 'data:' && request.type !== 'Document') continue
        if (csp && url.origin === server.origin && url.pathname === CSP_REPORTS) continue
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
        return runtime.length && (${hosted} || generated.length) ? { runtime, generated } : null
      })()`,
      'runtime and generated navigation'
    )
    if (hosted && listed.generated.length)
      throw new Error(`the hosted preview lists ${listed.generated.length} generated pages`)

    // The Showcase leads the runtime pages, as the HUD's leads its sections.
    const [overview, ...runtimePages] = listed.runtime
    if (overview !== COMPONENT_OVERVIEW || !runtimePages.length)
      throw new Error(
        `the sidebar lists the runtime pages ${listed.runtime.join(', ') || '(none)'}: ${COMPONENT_OVERVIEW} must lead them`
      )
    const examples = []
    for (const name of listed.runtime) {
      for (const width of [1280, 375]) {
        // At 375px the gallery page itself is a phone window too, so its header (the summary, the export names, the
        // width controls) must wrap within it.
        if (width === 375)
          await session.send('Emulation.setDeviceMetricsOverride', {
            width,
            height: 900,
            deviceScaleFactor: 1,
            mobile: false
          })
        else await session.send('Emulation.clearDeviceMetricsOverride')
        await open(`?component=${encodeURIComponent(name)}&width=${width}`)
        const drawn = await waitFor(
          session,
          `${CANVAS}?.querySelector('main')?.dataset.component === ${JSON.stringify(name)} && document.querySelector('iframe').clientWidth === ${width} && ${EXAMPLES}`,
          `the ${name} canvas at ${width}px`
        )
        for (const example of drawn) {
          if (example.error) throw new Error(`${name} ${example.id} failed: ${example.error}`)
          if (!example.drawn) throw new Error(`${name} ${example.id} draws nothing`)
        }
        await checkLayout(session, `${name} at ${width}px`)
        if (width === 375) {
          const page = await session.evaluate(
            `[document.documentElement.scrollWidth, innerWidth, [...document.querySelectorAll('header *')].filter((node) => node.getBoundingClientRect().right > innerWidth + 1).map((node) => node.tagName.toLowerCase() + ' ' + (node.textContent ?? '').trim().slice(0, 40))]`
          )
          if (page[0] > page[1] + 1 || page[2].length)
            throw new Error(
              `the ${name} gallery page is ${page[0]}px wide in a ${page[1]}px window: ${page[2].join('; ') || 'the canvas'}`
            )
        }
        if (name === overview) {
          const shown = await waitFor(
            session,
            SHOWCASE_PAGES,
            `the pages of the ${overview} page at ${width}px`
          )
          const names = shown.map((page) => page.name)
          if (names.join() !== runtimePages.join())
            throw new Error(
              `the ${overview} page at ${width}px draws ${names.join(', ') || 'no page'}; the sidebar lists ${runtimePages.join(', ')}`
            )
          for (const page of shown) {
            if (page.error) throw new Error(`${overview} ${page.name} failed: ${page.error}`)
            if (!page.drawn) throw new Error(`the ${overview} page draws nothing for ${page.name}`)
            // A canvas of its own is a viewport of the chosen width, as on the page itself.
            if (page.width !== null && page.width !== width)
              throw new Error(
                `the ${overview} page draws ${page.name} in a ${page.width}px canvas at ${width}px`
              )
          }
        } else if (width === 1280) examples.push(...drawn.map((example) => `${name}/${example.id}`))
      }
    }

    await session.send('Emulation.clearDeviceMetricsOverride')

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
      await checkLayout(session, `the ${name} foundations page`)
    }

    const hudPages = await session.evaluate(
      `[...document.querySelectorAll('nav a[data-source="hud"]')].map((a) => new URL(a.href).searchParams.get('hud')).filter(Boolean)`
    )
    const [showcase, ...pages] = hudPages
    if (!showcase || !pages.length)
      throw new Error(`the sidebar lists no HUD pages: ${hudPages.join(', ')}`)
    // A HUD page at a width: the page it shows, its window (size and the CSS that shapes it), the HUD scale inside
    // it, and what it draws.
    const hudPage = async (name, width) => {
      await open(`?hud=${encodeURIComponent(name)}&width=${width}`)
      return waitFor(
        session,
        `(() => {
          const doc = ${CANVAS}
          const main = doc?.querySelector('main')
          const frame = document.querySelector('iframe')
          const examples = main && frame.clientWidth === ${width} ? ${EXAMPLES} : null
          const style = getComputedStyle(frame)
          return examples && {
            hud: main.dataset.hud,
            height: frame.clientHeight,
            aspectRatio: style.aspectRatio,
            minBlockSize: style.minBlockSize,
            inner: [doc.defaultView.innerWidth, doc.defaultView.innerHeight],
            scale: doc.documentElement.style.getPropertyValue('--ui-scale'),
            examples
          }
        })()`,
        `the HUD ${name} page at ${width}px`
      )
    }
    // The window follows its own aspect ratio at 1280px and its minimum height at 375px; the scale inside is the one
    // hudScale.ts computes for that window, there with the HUD's lowest bound.
    const hudScale = readHudScale()
    for (const width of [1280, 375]) {
      const page = await hudPage(showcase, width)
      const height = expectedWindowHeight(width, page.aspectRatio, page.minBlockSize)
      if (page.height !== height)
        throw new Error(
          `the HUD canvas at ${width}px is ${page.height}px high; its aspect-ratio ${page.aspectRatio} and min-block-size ${page.minBlockSize} make ${height}px`
        )
      const scale = expectedHudScale(hudScale, ...page.inner)
      if (page.scale !== scale)
        throw new Error(
          `the HUD canvas's --ui-scale is ${page.scale || 'unset'} in a ${page.inner.join('x')} window; hudScale.ts gives ${scale}`
        )
      if (page.examples.length !== pages.length)
        throw new Error(
          `the ${showcase} page draws ${page.examples.length} pages; the sidebar lists ${pages.length}`
        )
      for (const example of page.examples) {
        if (example.error) throw new Error(`HUD ${example.id} failed: ${example.error}`)
        if (!example.drawn) throw new Error(`HUD ${example.id} draws nothing`)
      }
      await checkLayout(session, `the HUD ${showcase} page at ${width}px`)
    }
    // The HudScale section's live line shows the scale the HUD is drawn at, with hudScale.ts's bounds, and follows the
    // window when the width preset changes in place (the canvas resizes; it does not reload).
    const liveScale = (width) =>
      waitFor(
        session,
        `(() => {
          const doc = ${CANVAS}
          const frame = document.querySelector('iframe')
          const line = doc?.querySelector('output[aria-label="Live HUD scale"]')
          if (!line || frame.clientWidth !== ${width}) return null
          const view = doc.defaultView
          return { inner: [view.innerWidth, view.innerHeight], text: line.textContent }
        })()`,
        `the HudScale live scale at ${width}px`
      )
    const shownScale = async (width) => {
      const started = Date.now()
      for (;;) {
        const live = await liveScale(width)
        const expected = Number(expectedHudScale(hudScale, ...live.inner)).toFixed(3)
        if (live.text.endsWith(`×${expected}`)) return live
        if (Date.now() - started > TIMEOUT)
          throw new Error(
            `the HudScale section shows "${live.text}" in a ${live.inner.join('x')} window; hudScale.ts gives ×${expected}`
          )
        await delay(100)
      }
    }
    await hudPage('HudScale', 375)
    await shownScale(375)
    const widest = await session.evaluate(
      `(() => {
        const select = [...document.querySelectorAll('select')].find((node) => node.closest('label')?.textContent.includes('Width'))
        const option = [...select.options].filter((item) => item.value !== 'custom' && /\\d+px$/.test(item.label)).at(-1)
        select.value = option.value
        select.dispatchEvent(new Event('change', { bubbles: true }))
        return Number(option.label.match(/(\\d+)px$/)[1])
      })()`
    )
    if (widest === 375)
      throw new Error(
        'the width presets offer no width but 375px, so the HudScale step changes nothing'
      )
    await shownScale(widest)

    const alias = await hudPage('TextArea', 1280)
    if (alias.hud !== 'FormControls' || alias.examples.length !== 1)
      throw new Error(
        `?hud=TextArea opens ${alias.hud} with ${alias.examples.length} pages, not FormControls`
      )

    const generated = hosted ? 'Button' : listed.generated[0]
    await open(`?source=generated&component=${encodeURIComponent(generated)}`)
    if (hosted)
      await waitFor(
        session,
        `!document.querySelector('iframe') && (document.querySelector('[role="note"]')?.textContent ?? '').includes('Not in this hosted preview')`,
        'the note in place of the Figma reference'
      )
    else
      await waitFor(
        session,
        `${CANVAS}?.querySelector('main h1')?.textContent === ${JSON.stringify(generated)}`,
        `the generated ${generated} page inside the gallery`
      )
    const reviewRequests = server.requests.filter((request) =>
      request.path.includes('/dcl-ui-review/')
    )
    if (hosted && reviewRequests.length)
      throw new Error(
        `the hosted preview requested the generated Review: ${reviewRequests[0].path}`
      )

    if (csp) await delay(CSP_REPORT_WAIT)
    checkRequests()
    const failed = server.requests.filter((request) => request.status >= 400)
    if (failed.length)
      throw new Error(
        `${failed.length} requests failed, first ${failed[0].status} ${failed[0].path}`
      )
    if (session.runtimeErrors.length) throw new Error(`page error: ${session.runtimeErrors[0]}`)
    if (session.consoleErrors.length) throw new Error(`console error: ${session.consoleErrors[0]}`)
    summary = {
      entry: `${base}${hosted ? 'index.html' : 'gallery/'}`,
      hosted,
      runtimePages: runtimePages.length,
      examples: examples.length,
      foundations: foundations.length,
      hudPages: pages.length,
      generatedPages: listed.generated.length,
      generated,
      requests: server.requests.length
    }
  } catch (error) {
    // A check that fails under the policy names what the policy blocked, which is its cause.
    failure = { error: violation() ?? error }
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

function usage() {
  process.stderr.write('Usage: check-build <dist> [--prefix <path>] [--hosted] [--csp <policy>]\n')
  return 2
}

/** Command-line entry point. Returns the exit code. */
async function main(argv) {
  const [dist, ...rest] = argv
  let prefix = '/'
  let hosted = false
  let csp
  for (let at = 0; at < rest.length; at += 1) {
    if (rest[at] === '--hosted') hosted = true
    else if (rest[at] === '--prefix' && at + 1 < rest.length) prefix = rest[(at += 1)]
    else if (rest[at] === '--csp' && rest[at + 1]?.trim()) csp = rest[(at += 1)]
    else return usage()
  }
  if (!dist || dist.startsWith('--')) return usage()
  try {
    process.stdout.write(
      `${JSON.stringify(await checkGallery({ dist, prefix, hosted, csp }), null, 2)}\n`
    )
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

export { checkGallery, checkLayout, main }
