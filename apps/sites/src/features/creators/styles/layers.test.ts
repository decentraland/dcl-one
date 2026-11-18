// @vitest-environment node
import { readdirSync, readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const declared = /@layer ([\w\s,-]+);/.exec(readFileSync('index.html', 'utf8'))?.[1].split(/\s*,\s*/) ?? []

const stylesheets = (readdirSync('src', { recursive: true }) as string[])
  .map(path => `src/${path.replace(/\\/g, '/')}`)
  .filter(path => path.endsWith('.css'))

describe('cascade layers', () => {
  it('are ordered in index.html, resets under the shared components', () => {
    expect(declared).toEqual(['reset', 'foundations'])
  })

  it('are only the declared ones', () => {
    expect(stylesheets.length).toBeGreaterThan(0)
    const undeclared = stylesheets.flatMap(path =>
      [...readFileSync(path, 'utf8').matchAll(/@layer\s+([\w\s,-]+?)\s*[;{]/g)]
        .flatMap(match => match[1].split(/\s*,\s*/))
        .filter(name => !declared.includes(name))
        .map(name => `${path}: ${name}`)
    )
    expect(undeclared, 'A new layer joins the order in index.html first').toEqual([])
  })
})
