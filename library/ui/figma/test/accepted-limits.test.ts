import { readFileSync } from 'node:fs'
import path from 'node:path'
import { describe, expect, it } from 'vitest'
import { componentRegistryEntries } from '../component-generation.mjs'

const ui = path.resolve(import.meta.dirname, '../..')
const MARKER = '<!-- navbar-accepted-limits -->'

// The bullet list right after the marker: "- `id`: text", with only blank lines before it. Any other line before
// the first bullet is refused, so a broken list never picks up a later bullet elsewhere in the README.
function readmeLimits(markdown: string): { id: string; text: string }[] {
  const lines = markdown.split(/\r?\n/)
  const start = lines.findIndex((line) => line.trim() === MARKER)
  if (start < 0) throw new Error('README has no accepted-limits marker')
  const limits: { id: string; text: string }[] = []
  for (const line of lines.slice(start + 1)) {
    const match = /^- `([^`]+)`: (.+)$/.exec(line)
    if (match) limits.push({ id: match[1]!, text: match[2]! })
    else if (limits.length) break
    else if (line.trim()) throw new Error(`README accepted-limits list starts with: ${line}`)
  }
  return limits
}

const sample = [
  { id: 'logo-size', text: 'The logo is pending.' },
  { id: 'avatar', text: 'The avatar is a placeholder.' }
]

describe('the accepted limits of generated public components', () => {
  // NavBar, the last generated public component, is hand-written: its limits live in docs/ui-components.md.
  it('should record no limits in the component registry', () => {
    const registry = componentRegistryEntries(
      JSON.parse(readFileSync(path.join(ui, 'components/promotion.json'), 'utf8'))
    )
    expect(registry.limits).toEqual({})
  })

  it('should keep no accepted-limits list in the README', () => {
    expect(readFileSync(path.join(ui, 'README.md'), 'utf8')).not.toContain(MARKER)
  })

  it.each([
    ['drops', (list: { id: string; text: string }[]) => list.slice(1)],
    [
      'rewords',
      (list: { id: string; text: string }[]) =>
        list.map((l, i) => (i ? l : { ...l, text: 'Reworded.' }))
    ]
  ])('should fail the comparison when the README %s a limit', (_label, change) => {
    const readme = `${MARKER}\n\n${change(sample)
      .map((l) => `- \`${l.id}\`: ${l.text}`)
      .join('\n')}\n`
    expect(readmeLimits(readme)).not.toEqual(sample)
  })

  it('should read back an unchanged list', () => {
    const readme = `${MARKER}\n\n${sample.map((l) => `- \`${l.id}\`: ${l.text}`).join('\n')}\n`
    expect(readmeLimits(readme)).toEqual(sample)
  })

  it('should refuse a README without the marker', () => {
    expect(() => readmeLimits('no marker')).toThrow('no accepted-limits marker')
  })

  it('should read a CRLF checkout of the list', () => {
    expect(readmeLimits(`${MARKER}\r\n\r\n- \`a\`: One.\r\n- \`b\`: Two.\r\n\r\nAfter.`)).toEqual([
      { id: 'a', text: 'One.' },
      { id: 'b', text: 'Two.' }
    ])
  })

  it('should refuse prose between the marker and the list, instead of reading a later bullet', () => {
    expect(() => readmeLimits(`${MARKER}\n\nSome prose.\n\n- \`a\`: Elsewhere.`)).toThrow(
      'list starts with: Some prose.'
    )
  })

  // The nearest false positive: a list ends at its first non-bullet line, blank or not.
  it('should stop at the end of the list without reading later bullets', () => {
    expect(readmeLimits(`${MARKER}\n- \`a\`: One.\n\n- \`b\`: Later list.`)).toEqual([
      { id: 'a', text: 'One.' }
    ])
  })
})

describe('componentRegistryEntries', () => {
  const limit = { id: 'logo-size', text: 'The logo is pending.' }

  it('should accept names and public entries with limits', () => {
    expect(
      componentRegistryEntries({
        components: ['Button', { name: 'NavBar', limits: [limit] }],
        internal: ['Menu']
      })
    ).toEqual({ components: ['Button', 'NavBar'], internal: ['Menu'], limits: { NavBar: [limit] } })
  })

  it.each([
    ['a duplicate id', { name: 'NavBar', limits: [limit, { ...limit, text: 'Other.' }] }],
    ['an empty text', { name: 'NavBar', limits: [{ id: 'a', text: '' }] }],
    ['padded text', { name: 'NavBar', limits: [{ id: 'a', text: ' padded' }] }],
    ['an over-long text', { name: 'NavBar', limits: [{ id: 'a', text: 'x'.repeat(301) }] }],
    ['a link', { name: 'NavBar', limits: [{ id: 'a', text: 'see https://example.com' }] }],
    ['a control character', { name: 'NavBar', limits: [{ id: 'a', text: 'line\nbreak' }] }],
    ['a C1 control', { name: 'NavBar', limits: [{ id: 'a', text: 'next\u0085line' }] }],
    ['a bidi override', { name: 'NavBar', limits: [{ id: 'a', text: 'abc\u202Edef' }] }],
    ['a zero-width space', { name: 'NavBar', limits: [{ id: 'a', text: 'zero\u200Bwidth' }] }],
    ['a paragraph separator', { name: 'NavBar', limits: [{ id: 'a', text: 'a\u2029b' }] }],
    ['a www link', { name: 'NavBar', limits: [{ id: 'a', text: 'see www.example.org' }] }],
    ['a mailto link', { name: 'NavBar', limits: [{ id: 'a', text: 'mailto:someone' }] }],
    ['markup', { name: 'NavBar', limits: [{ id: 'a', text: 'an <img> tag' }] }],
    ['a name that is not a string', { name: 5, limits: [limit] }],
    ['no name', { limits: [limit] }],
    ['a name that is not an identifier', { name: '../NavBar', limits: [limit] }],
    [
      'more than ten limits',
      {
        name: 'NavBar',
        limits: Array.from({ length: 11 }, (_, i) => ({ id: `l${i}`, text: 'Pending.' }))
      }
    ],
    ['an id that is not kebab-case', { name: 'NavBar', limits: [{ id: 'Logo_Size', text: 'x.' }] }],
    ['an empty limits list', { name: 'NavBar', limits: [] }],
    ['an unknown key', { name: 'NavBar', limits: [limit], extra: true }],
    ['an unknown limit key', { name: 'NavBar', limits: [{ ...limit, url: 'x' }] }]
  ])('should refuse %s', (_label, entry) => {
    expect(() => componentRegistryEntries({ components: [entry] })).toThrow()
  })

  // The nearest false positives of the stricter text rule and the cap.
  it.each([
    ['ordinary punctuation and units', 'The logo is 60px on desktop; alignment (40px) is pending.'],
    ['a colon that is not a scheme', 'Note: the avatar is a placeholder.'],
    ['a non-ASCII letter', 'Résumé views are pending.']
  ])('should accept %s', (_label, text) => {
    expect(
      componentRegistryEntries({ components: [{ name: 'NavBar', limits: [{ id: 'a', text }] }] })
        .limits.NavBar
    ).toEqual([{ id: 'a', text }])
  })

  it('should accept exactly ten limits', () => {
    const limits = Array.from({ length: 10 }, (_, i) => ({ id: `l${i}`, text: 'Pending.' }))
    expect(
      componentRegistryEntries({ components: [{ name: 'NavBar', limits }] }).limits.NavBar
    ).toHaveLength(10)
  })

  it.each([
    ['a public name twice', { components: ['NavBar', { name: 'NavBar', limits: [limit] }] }],
    ['a name both public and internal', { components: ['NavBar'], internal: ['NavBar'] }],
    ['a name twice in another case', { components: ['NavBar', 'navbar'] }]
  ])('should refuse %s', (_label, registry) => {
    expect(() => componentRegistryEntries(registry)).toThrow('Duplicate component registry name')
  })

  // The nearest false positive: distinct names, one with limits, are kept apart.
  it('should accept distinct names', () => {
    expect(
      componentRegistryEntries({ components: ['Button', { name: 'NavBar', limits: [limit] }] })
        .components
    ).toEqual(['Button', 'NavBar'])
  })

  it('should refuse limits on an internal name', () => {
    expect(() =>
      componentRegistryEntries({ components: [], internal: [{ name: 'NavBar', limits: [limit] }] })
    ).toThrow('Internal component registry entries are names only')
  })
})
