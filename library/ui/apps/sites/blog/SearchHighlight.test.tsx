import { Fragment, type ReactNode } from 'react'
import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import captured from './fixtures/highlight-source-output.json'
import { SearchHighlight } from './SearchHighlight'

// R5-2: ours against the source's rendered highlights, captured for adversarial inputs (provenance in
// the fixture). Both sides are compared as the DOM serializes them.
const html = (value: string) =>
  render(<SearchHighlight value={value} />).container.firstElementChild!.innerHTML

// The one listed deviation: DOMPurify keeps data-* attributes on em and mark; ours renders none.
const DEVIATIONS: Record<string, string> = {
  '<mark data-a="1" id="m">m</mark>': '<mark>m</mark>'
}

describe('SearchHighlight against the source render', () => {
  it.each(captured.cases.filter((c) => !(c.input in DEVIATIONS)).map((c) => [c.input, c.html]))(
    'renders %j as the source does',
    (input, expected) => {
      expect(html(input)).toBe(expected)
    }
  )

  it.each(Object.entries(DEVIATIONS))(
    'renders %j without the data attribute the source keeps (listed deviation)',
    (input, ours) => {
      const source = captured.cases.find((c) => c.input === input)!.html
      expect(source).toBe('<mark data-a="1">m</mark>')
      expect(html(input)).toBe(ours)
    }
  )

  // Listed deviation (c): a valid HTML name outside our 22-entry table stays literal text (the browser
  // would decode it). Measured: the 471-post corpus uses only amp, lt and gt in titles, descriptions
  // and body text, all in the table. An invalid name stays literal in the source too (captured case).
  it('renders a valid named reference outside its table as literal text (listed deviation)', () => {
    expect(html('caf&eacute; &hearts;')).toBe('caf&amp;eacute; &amp;hearts;')
    expect(html('&amp; &lt; &gt;')).toBe('&amp; &lt; &gt;')
  })

  it('never renders an element other than em and mark, nor any attribute', () => {
    for (const { input } of captured.cases) {
      const root = render(<SearchHighlight value={input} />).container.firstElementChild!
      for (const element of root.querySelectorAll('*')) {
        expect(['EM', 'MARK']).toContain(element.tagName)
        expect(element.attributes).toHaveLength(0)
      }
    }
  })

  it('covers each adversarial class R5-2 names', () => {
    const inputs = captured.cases.map((c) => c.input)
    const has = (pattern: RegExp) => inputs.some((input) => pattern.test(input))
    expect(has(/<b>|<div>|<span>/)).toBe(true) // disallowed tags with text inside
    expect(has(/<script>/) && has(/<style>/)).toBe(true)
    expect(has(/<em>[^<]*<em>/) && has(/^<em>[^<]*$/) && has(/^[^<]*<\/em>/)).toBe(true) // nested and unbalanced
    expect(has(/<em [^>]*=/)).toBe(true) // attributes on em
    expect(has(/&amp;lt;/) && has(/&lt;em&gt;/)).toBe(true) // entities and double-encoded entities
    expect(has(/<EM>/) && has(/<Mark>/)).toBe(true) // mixed case
  })
})

// The reference: the HTML parser's tree (parse5 here, the browser's in the source), walked with the
// source's sanitizer decisions (em and mark kept bare; DOMPurify's content-dropping elements; noscript
// content kept, as captured).
const REFERENCE_DROPS = new Set([
  'script',
  'style',
  'iframe',
  'xmp',
  'noembed',
  'noframes',
  'title',
  'template',
  'audio',
  'video',
  'svg',
  'math',
  'plaintext'
])
function reference(value: string): string {
  const walk = (node: Node, key: number): ReactNode => {
    if (node.nodeType === Node.TEXT_NODE) return node.nodeValue
    if (node.nodeType !== Node.ELEMENT_NODE) return null
    const name = (node as Element).localName.toLowerCase()
    if (REFERENCE_DROPS.has(name)) return null
    const children = [...node.childNodes].map(walk)
    if (name === 'em' || name === 'mark') {
      const Tag = name
      return <Tag key={key}>{children}</Tag>
    }
    return <Fragment key={key}>{children}</Fragment>
  }
  const body = new DOMParser().parseFromString(`<!doctype html><body>${value}`, 'text/html').body
  return render(<div>{[...body.childNodes].map(walk)}</div>).container.firstElementChild!.innerHTML
}

// Deterministic generator (mulberry32), so a failure reproduces.
function random(seed: number) {
  return () => {
    seed |= 0
    seed = (seed + 0x6d2b79f5) | 0
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed)
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}
// Inline and void tags only: a block tag (p, div, li...) inside a highlight closes an open mark in the
// browser's tree builder, which ours does not emulate (listed deviation, see SearchHighlight.tsx).
const FRAGMENTS = [
  '<em>',
  '</em>',
  '<mark>',
  '</mark>',
  '<EM>',
  '</Mark>',
  '<em class="a">',
  '<b>',
  '</b>',
  '<span>',
  '</span>',
  '<i>',
  '</i>',
  '<br>',
  '<img src="x">',
  '<script>x<em>s</em></script>',
  '<style>.a{}</style>',
  '<template><em>t</em></template>',
  '<textarea><em>&amp;</em></textarea>',
  '<!-- c -->',
  '<svg><circle/></svg>',
  '<svg><em>out</em></svg>',
  '<math><mi>x</mi></math>',
  'word',
  ' ',
  'a < b',
  '&amp;',
  '&lt;em&gt;',
  '&amp;lt;',
  '&#39;',
  '&#x41;',
  '&nbsp;',
  '&copy;',
  '&unknown;',
  'café'
]

describe('SearchHighlight against the parser reference', () => {
  it('matches element for element on 500 generated values', () => {
    const next = random(52)
    for (let n = 0; n < 500; n++) {
      const length = 1 + Math.floor(next() * 8)
      const value = Array.from(
        { length },
        () => FRAGMENTS[Math.floor(next() * FRAGMENTS.length)]
      ).join('')
      expect([value, html(value)]).toEqual([value, reference(value)])
    }
  })
})
