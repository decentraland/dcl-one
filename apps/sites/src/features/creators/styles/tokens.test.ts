// @vitest-environment node
import '@/data/lib/creators/test/setup'
import { readdirSync, readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const TOKENS = 'src/features/creators/styles/tokens.css'
// The creators code: its components, and the logic layer that holds the theme.
const ROOTS = ['src/features/creators', 'src/data/lib/creators']

const read = (path: string) => readFileSync(path, 'utf8')
const sources = ROOTS.flatMap(root =>
  (readdirSync(root, { recursive: true }) as string[]).map(path => `${root}/${path.replace(/\\/g, '/')}`)
).filter(path => /\.(css|tsx?)$/.test(path) && !/\.test\.tsx?$/.test(path) && path !== TOKENS)

// Set by Sites' frame around the page, not by this app.
const FROM_SITES = new Set(['--site-header-h'])
// The shared foundations' tokens, loaded once by Sites.
const SHARED = '--dcl-'

const tokens = [...read(TOKENS).matchAll(/^\s*(--[\w-]+):/gm)].map(match => match[1])
const used = new Set(sources.flatMap(path => [...read(path).matchAll(/var\((--[\w-]+)/g)].map(match => match[1])))
// Custom properties a stylesheet declares for its own elements, or a component sets on `style` at runtime.
const local = new Set(
  sources.flatMap(path =>
    [...read(path).matchAll(/(--[\w-]+)\s*:|['"](--[\w-]+)['"]/g)].map(match => match[1] ?? match[2])
  )
)

describe('tokens.css', () => {
  it('declares every custom property the app reads', () => {
    expect(used.size).toBeGreaterThan(0)
    const undeclared = [...used].filter(name => !tokens.includes(name) && !local.has(name) && !FROM_SITES.has(name) && !name.startsWith(SHARED))
    expect(undeclared, 'Declare the token in tokens.css, or fix the name').toEqual([])
  })

  it('holds only tokens something reads', () => {
    expect(tokens.length).toBeGreaterThan(0)
    expect(
      tokens.filter(name => !used.has(name)),
      'Delete the unused token'
    ).toEqual([])
  })
})
