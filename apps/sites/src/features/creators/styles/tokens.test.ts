// @vitest-environment node
import { readdirSync, readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const TOKENS = 'src/styles/tokens.css'

const read = (path: string) => readFileSync(path, 'utf8')
const sources = (readdirSync('src', { recursive: true }) as string[])
  .map(path => `src/${path.replace(/\\/g, '/')}`)
  .filter(path => /\.(css|tsx?)$/.test(path) && !/\.spec\.tsx?$/.test(path) && path !== TOKENS)

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
    const undeclared = [...used].filter(name => !tokens.includes(name) && !local.has(name))
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
