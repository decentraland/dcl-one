import { readFileSync } from 'node:fs'
import path from 'node:path'
import { describe, expect, it } from 'vitest'
import { componentRegistryEntries } from '../component-generation.mjs'

const ui = path.resolve(import.meta.dirname, '../..')
const MARKER = '<!-- navbar-accepted-limits -->'

// The bullet list right after the marker: "- `id`: text".
function readmeLimits(markdown: string): { id: string; text: string }[] {
  const lines = markdown.split('\n')
  const start = lines.findIndex((line) => line.trim() === MARKER)
  if (start < 0) throw new Error('README has no accepted-limits marker')
  const limits: { id: string; text: string }[] = []
  for (const line of lines.slice(start + 1)) {
    const match = /^- `([^`]+)`: (.+)$/.exec(line)
    if (match) limits.push({ id: match[1]!, text: match[2]! })
    else if (limits.length && line.trim()) break
  }
  return limits
}

describe('the accepted NavBar limits', () => {
  it('should be the same list, by id and text, in the README and the component registry', () => {
    const registry = componentRegistryEntries(
      JSON.parse(readFileSync(path.join(ui, 'components/promotion.json'), 'utf8'))
    )
    expect(readmeLimits(readFileSync(path.join(ui, 'README.md'), 'utf8'))).toEqual(
      registry.limits.NavBar
    )
  })

  it.each([
    ['drops', (list: { id: string; text: string }[]) => list.slice(1)],
    [
      'rewords',
      (list: { id: string; text: string }[]) =>
        list.map((l, i) => (i ? l : { ...l, text: 'Reworded.' }))
    ]
  ])('should fail the comparison when the README %s a limit', (_label, change) => {
    const registry = componentRegistryEntries(
      JSON.parse(readFileSync(path.join(ui, 'components/promotion.json'), 'utf8'))
    ).limits.NavBar!
    const readme = `${MARKER}\n\n${change(registry)
      .map((l) => `- \`${l.id}\`: ${l.text}`)
      .join('\n')}\n`
    expect(readmeLimits(readme)).not.toEqual(registry)
  })

  it('should refuse a README without the marker', () => {
    expect(() => readmeLimits('no marker')).toThrow('no accepted-limits marker')
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
    ['an id that is not kebab-case', { name: 'NavBar', limits: [{ id: 'Logo_Size', text: 'x.' }] }],
    ['an empty limits list', { name: 'NavBar', limits: [] }],
    ['an unknown key', { name: 'NavBar', limits: [limit], extra: true }],
    ['an unknown limit key', { name: 'NavBar', limits: [{ ...limit, url: 'x' }] }]
  ])('should refuse %s', (_label, entry) => {
    expect(() => componentRegistryEntries({ components: [entry] })).toThrow()
  })

  it('should refuse limits on an internal name', () => {
    expect(() =>
      componentRegistryEntries({ components: [], internal: [{ name: 'NavBar', limits: [limit] }] })
    ).toThrow('Internal component registry entries are names only')
  })
})
