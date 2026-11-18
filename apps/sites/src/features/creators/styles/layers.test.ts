// @vitest-environment node
import '@/data/lib/creators/test/setup'
import { readdirSync, readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

// The creators code: its components, and the logic layer that holds the theme.
const ROOTS = ['src/features/creators', 'src/data/lib/creators']

const declared = /@layer ([\w\s,-]+);/.exec(readFileSync('src/features/creators/styles/index.css', 'utf8'))?.[1].split(/\s*,\s*/) ?? []

const stylesheets = ROOTS.flatMap(root =>
  (readdirSync(root, { recursive: true }) as string[]).map(path => `${root}/${path.replace(/\\/g, '/')}`)
).filter(path => path.endsWith('.css'))

describe('cascade layers', () => {
  it('are ordered in the global stylesheet, resets under the shared components', () => {
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
    expect(undeclared, 'A new layer joins the order in styles/index.css first').toEqual([])
  })
})
