import type { ReactNode } from 'react'

// A search hit's highlighted text, as source renders it (components/blog/Search/sanitizeHighlight.ts:
// DOMPurify 3.4.3 with em and mark only, set as innerHTML). Ours tokenizes the value and builds React
// elements, so it renders the same on the server and in the browser, with no HTML set. The tokenizer
// follows the HTML parsing rules these values exercise; equivalence with the source is proven on
// captured adversarial inputs (blog plan R5-2), and with a parser-based reference on generated ones.
// Listed deviations: DOMPurify keeps data-* attributes on em and mark (ours renders none); named
// character references outside NAMED stay literal (the browser decodes all of HTML's); a block tag
// (p, div, li...) mis-nested with em or mark can end a mark, or split an em, in the browser's tree
// builder, which ours does not emulate (the same text shows; its emphasis can differ).

type Highlight = string | { tag: 'em' | 'mark'; children: Highlight[] }

const ALLOWED = new Set(['em', 'mark'])
// Raw text elements: their content is never markup. RCDATA ones (textarea, title) decode references.
const RAW_DROPPED = new Set(['script', 'style', 'iframe', 'xmp', 'noembed', 'noframes', 'title'])
const RAW_KEPT = new Set(['textarea'])
// Elements whose content DOMPurify drops (its default FORBID_CONTENTS, as they parse in a body).
const CONTENT_DROPPED = new Set(['template', 'audio', 'video', 'svg', 'math'])
// In svg or math, these start tags leave foreign content (HTML parsing, "any other start tag").
const BREAKOUT = new Set(
  'b big blockquote body br center code dd div dl dt em embed h1 h2 h3 h4 h5 h6 head hr i img li listing menu meta nobr ol p pre ruby s small span strong strike sub sup table tt u ul var'.split(
    ' '
  )
)

const NAMED: Record<string, string> = {
  amp: '&',
  lt: '<',
  gt: '>',
  quot: '"',
  apos: "'",
  nbsp: ' ',
  copy: '©',
  reg: '®',
  trade: '™',
  hellip: '…',
  mdash: '—',
  ndash: '–',
  lsquo: '‘',
  rsquo: '’',
  ldquo: '“',
  rdquo: '”',
  laquo: '«',
  raquo: '»',
  middot: '·',
  bull: '•',
  deg: '°',
  euro: '€'
}

function decode(text: string): string {
  return text.replace(/&(#[xX][0-9a-fA-F]+|#\d+|[a-zA-Z][a-zA-Z0-9]*);/g, (whole, ref: string) => {
    if (ref[0] !== '#') return NAMED[ref] ?? whole
    const code =
      ref[1] === 'x' || ref[1] === 'X' ? parseInt(ref.slice(2), 16) : parseInt(ref.slice(1), 10)
    if (code === 0 || code > 0x10ffff || (code >= 0xd800 && code <= 0xdfff)) return '�'
    return String.fromCodePoint(code)
  })
}

const TAG = /^<(\/?)([a-zA-Z][^\s/>]*)(?:[^>"']|"[^"]*"|'[^']*')*>/

/** The value as text and em/mark nodes. */
export function parseHighlight(value: string): Highlight[] {
  const root: Highlight[] = []
  const stack: { tag: 'em' | 'mark'; children: Highlight[] }[] = []
  const out = () => (stack.length ? stack[stack.length - 1]!.children : root)
  const text = (raw: string) => {
    if (!raw) return
    const target = out()
    const last = target[target.length - 1]
    if (typeof last === 'string') target[target.length - 1] = last + raw
    else target.push(raw)
  }
  let i = 0
  // Inside svg/math or template: the open count of that element, so its content is skipped.
  let skipping: { name: string; depth: number } | null = null
  while (i < value.length) {
    const rest = value.slice(i)
    if (rest.startsWith('<!--')) {
      const end = value.indexOf('-->', i + 4)
      i = end < 0 ? value.length : end + 3
      continue
    }
    if (/^<[!?]/.test(rest) || /^<\/[^a-zA-Z>]/.test(rest)) {
      // Bogus comment (doctype, processing instruction, "</ x>"): dropped up to its ">".
      const end = value.indexOf('>', i)
      i = end < 0 ? value.length : end + 1
      continue
    }
    const tag = TAG.exec(rest)
    if (!tag) {
      const next = value.indexOf('<', i + 1)
      const chunk = value.slice(i, next < 0 ? value.length : next)
      if (!skipping) text(decode(chunk))
      i += chunk.length
      continue
    }
    i += tag[0].length
    const closing = tag[1] === '/'
    const name = tag[2]!.toLowerCase()
    if (skipping) {
      const foreign = skipping.name === 'svg' || skipping.name === 'math'
      if (foreign && !closing && BREAKOUT.has(name)) {
        skipping = null
      } else {
        if (name === skipping.name) skipping.depth += closing ? -1 : 1
        if (skipping.depth === 0) skipping = null
        continue
      }
    }
    if (closing) {
      if (!ALLOWED.has(name)) continue
      const at = stack.map((open) => open.tag).lastIndexOf(name as 'em' | 'mark')
      if (at >= 0) stack.length = at
      continue
    }
    if (RAW_DROPPED.has(name) || RAW_KEPT.has(name)) {
      const close = new RegExp(`</${name}[\\s/>]`, 'i').exec(value.slice(i))
      const content = value.slice(i, close ? i + close.index : value.length)
      if (RAW_KEPT.has(name)) text(decode(content))
      i = close ? value.indexOf('>', i + close.index) + 1 : value.length
      continue
    }
    if (name === 'plaintext') break
    if (CONTENT_DROPPED.has(name)) {
      skipping = { name, depth: 1 }
      continue
    }
    if (ALLOWED.has(name)) {
      const element = { tag: name as 'em' | 'mark', children: [] as Highlight[] }
      out().push(element)
      stack.push(element)
    }
  }
  return root
}

function toReact(nodes: Highlight[]): ReactNode[] {
  return nodes.map((node, index) => {
    if (typeof node === 'string') return node
    const Tag = node.tag
    return <Tag key={index}>{toReact(node.children)}</Tag>
  })
}

/** The highlighted value's nodes, with no element around them. */
export function HighlightText({ value }: { value: string }) {
  return <>{toReact(parseHighlight(value))}</>
}

export function SearchHighlight({ value, className }: { value: string; className?: string }) {
  return <div className={className}>{toReact(parseHighlight(value))}</div>
}
