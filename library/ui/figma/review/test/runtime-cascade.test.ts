import { after, before, describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import process from 'node:process'
import { pathToFileURL } from 'node:url'
import { findBrowser, openSession } from '../scripts/capture-browser.mjs'

const review = path.resolve(import.meta.dirname, '..')
const library = path.resolve(review, '../..')
const runtimeCss = path.join(review, 'src/components/figma-module-runtime.module.css')
const textStylesCss = path.join(review, 'src/generated/tokens/text-styles.module.css')
const navBarCss = path.join(review, 'src/generated/components/nav-bar/NavBar.module.css')
// The consumer font contract: the library declares the Inter family; the consumer loads its faces.
const fontCss = ['400', '600', '700'].map((weight) =>
  path.join(library, `node_modules/@fontsource/inter/${weight}.css`)
)
// The signed-out NavBar's Sign In label and the explorer/label-caps text style it composes.
const labelClasses = 'node v0n1_0_0_0 text6578706c6f7265722f6c6162656c2d63617073'

type Session = Awaited<ReturnType<typeof openSession>>
type Send = (method: string, params?: Record<string, unknown>) => Promise<unknown>
let session: Session
let folder: string

// Loads the stylesheets in the given order under a serif page, waits for the fonts, and returns the label's computed
// font family and the platform fonts the browser actually used to draw it.
async function renderLabel(stylesheets: (string | { css: string })[], name: string) {
  const links = stylesheets
    .map((sheet) =>
      typeof sheet === 'string'
        ? `<link rel="stylesheet" href="${pathToFileURL(sheet).href}">`
        : `<style>${sheet.css}</style>`
    )
    .join('')
  const page = path.join(folder, `${name}.html`)
  await writeFile(
    page,
    `<!doctype html><html><head>${links}<style>body{font-family:serif}</style></head>` +
      `<body><span id="label" class="${labelClasses}">Sign In</span></body></html>`
  )
  const send = session.send as Send
  await send('Page.navigate', { url: pathToFileURL(page).href })
  const { result } = (await send('Runtime.evaluate', {
    expression: `new Promise((resolve) => {
      const done = () => document.fonts.ready.then(() =>
        resolve(getComputedStyle(document.getElementById('label')).fontFamily))
      document.readyState === 'complete' ? done() : addEventListener('load', done, { once: true })
    })`,
    awaitPromise: true,
    returnByValue: true
  })) as { result: { value: string } }
  const { root } = (await send('DOM.getDocument')) as { root: { nodeId: number } }
  const { nodeId } = (await send('DOM.querySelector', {
    nodeId: root.nodeId,
    selector: '#label'
  })) as { nodeId: number }
  const { fonts } = (await send('CSS.getPlatformFontsForNode', { nodeId })) as {
    fonts: { familyName: string; isCustomFont: boolean }[]
  }
  // A custom font is a loaded @font-face, not a font installed on the machine.
  return {
    family: result.value,
    platformFonts: fonts.map(
      (font) => `${font.familyName}${font.isCustomFont ? ' (web font)' : ''}`
    )
  }
}

describe('when a consumer bundle loads the generated stylesheets in either order', () => {
  before(async () => {
    folder = await mkdtemp(path.join(os.tmpdir(), 'runtime-cascade-'))
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
    const send = session.send as Send
    await send('Page.enable')
    await send('DOM.enable')
    await send('CSS.enable')
  })
  after(async () => {
    await session?.close()
    await rm(folder, { recursive: true, force: true })
  })

  it('should draw the NavBar label in Inter when the runtime loads first', async () => {
    const label = await renderLabel(
      [...fontCss, runtimeCss, textStylesCss, navBarCss],
      'runtime-first'
    )
    assert.match(label.family, /^Inter\b|^"Inter"/)
    assert.equal(label.platformFonts.length, 1)
    assert.match(label.platformFonts[0]!, /^Inter\b.* \(web font\)$/)
  })

  it('should draw the NavBar label in Inter when the runtime loads last', async () => {
    const label = await renderLabel(
      [...fontCss, navBarCss, textStylesCss, runtimeCss],
      'runtime-last'
    )
    assert.match(label.family, /^Inter\b|^"Inter"/)
    assert.equal(label.platformFonts.length, 1)
    assert.match(label.platformFonts[0]!, /^Inter\b.* \(web font\)$/)
  })

  it('should fall back to the page font if the runtime reset were unlayered and loaded last (control)', async () => {
    const unlayered = (await readFile(runtimeCss, 'utf8'))
      .replace(/@layer dcl\.reset, dcl\.foundations, dcl\.components;/, '')
      .replace(/@layer dcl\.reset \{([\s\S]*)\}\s*$/, '$1')
    assert.ok(!unlayered.includes('@layer'))
    const label = await renderLabel(
      [...fontCss, navBarCss, textStylesCss, { css: unlayered }],
      'control'
    )
    assert.equal(label.family, 'serif')
  })

  it('should draw no Inter web font when the consumer loads no Inter faces (font contract)', async () => {
    const label = await renderLabel([runtimeCss, textStylesCss, navBarCss], 'no-faces')
    assert.match(label.family, /^Inter\b|^"Inter"/)
    assert.ok(label.platformFonts.every((font) => !font.endsWith('(web font)')))
  })
})
