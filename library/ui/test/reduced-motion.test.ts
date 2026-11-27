import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

// These stylesheets put every transition and animation inside `@media (prefers-reduced-motion: no-preference)`,
// as docs/ui-standards.md asks of a shared component: outside that query they may only turn motion off. Vitest
// runs from library/ui, its config's root.
const STYLESHEETS = [
  'components/WearableCard/WearableCard.module.css',
  'components/EquippedItemCard/EquippedItemCard.module.css',
  'components/PagedRail/PagedRail.module.css'
]
const QUERY = '@media (prefers-reduced-motion: no-preference)'

/** The stylesheet's text inside its no-preference blocks, and the rest. */
function split(css: string): { inside: string; outside: string } {
  let inside = ''
  let outside = ''
  let at = 0
  for (let start = css.indexOf(QUERY); start >= 0; start = css.indexOf(QUERY, at)) {
    outside += css.slice(at, start)
    let end = css.indexOf('{', start)
    let depth = 0
    do {
      if (css[end] === '{') depth += 1
      else if (css[end] === '}') depth -= 1
      end += 1
    } while (depth > 0 && end < css.length)
    inside += css.slice(start, end)
    at = end
  }
  return { inside, outside: outside + css.slice(at) }
}

/** Every transition and animation declaration of the text, comments left out, as `property: value`. */
const motion = (text: string): string[] =>
  [
    ...text
      .replace(/\/\*[\s\S]*?\*\//g, '')
      .matchAll(/(?:^|[\s;{])((?:transition|animation)(?:-[a-z]+)*)\s*:\s*([^;}]+)/g)
  ].map(([, property = '', value = '']) => `${property}: ${value.trim().replace(/\s+/g, ' ')}`)

const read = (file: string) => split(readFileSync(join(process.cwd(), file), 'utf8'))

describe.each(STYLESHEETS)('%s', (file) => {
  it('should hold its transitions or animations inside the no-preference query', () => {
    expect(motion(read(file).inside).length).toBeGreaterThan(0)
  })

  it('should only turn motion off outside that query', () => {
    expect(
      motion(read(file).outside).filter((declaration) => !declaration.endsWith(': none'))
    ).toEqual([])
  })
})

describe('EquippedItemCard', () => {
  it("should turn off the shared Button's transition on its link outside the query", () => {
    const { outside } = read('components/EquippedItemCard/EquippedItemCard.module.css')
    const start = outside.indexOf('\n.shopBtn {')
    expect(start).toBeGreaterThanOrEqual(0)
    expect(motion(outside.slice(start, outside.indexOf('}', start)))).toEqual(['transition: none'])
  })
})

describe('the split', () => {
  it('should keep a nested rule and what follows the query apart', () => {
    const { inside, outside } = split(
      `.a { color: red; }\n${QUERY} {\n  .b { transition: opacity 1s; }\n}\n.c { animation: none; }`
    )
    expect(motion(inside)).toEqual(['transition: opacity 1s'])
    expect(motion(outside)).toEqual(['animation: none'])
  })

  it('should leave a stylesheet without the query whole outside it', () => {
    const { inside, outside } = split('.a { transition: none; }')
    expect(inside).toBe('')
    expect(motion(outside)).toEqual(['transition: none'])
  })
})
