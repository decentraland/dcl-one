const LOCALE = 'en'

type Node =
  | string
  | { kind: 'arg'; name: string }
  | { kind: 'number'; name: string }
  | { kind: 'choice'; name: string; plural: boolean; arms: Record<string, Node[]> }
  | { kind: 'tag'; name: string; children: Node[] }

export type FormatValue = string | number | ((chunks: never[]) => unknown)

/** Parses the ICU subset the copy uses: `{name}`, `{n, number}`, `{n, plural, …}`, `{v, select, …}` and `<tag>…</tag>`. */
function parse(source: string): Node[] {
  let at = 0

  const sequence = (closer: '}' | '<' | undefined): Node[] => {
    const nodes: Node[] = []
    let text = ''
    const flush = () => {
      if (text) nodes.push(text)
      text = ''
    }
    while (at < source.length) {
      const char = source[at]
      if (char === '}' && closer === '}') break
      if (char === '<' && source[at + 1] === '/' && closer === '<') break
      const tag = char === '<' ? /^<(\w+)>/.exec(source.slice(at)) : null
      if (tag) {
        flush()
        at += tag[0].length
        const children = sequence('<')
        at += `</${tag[1]}>`.length
        nodes.push({ kind: 'tag', name: tag[1], children })
      } else if (char === '{') {
        flush()
        at++
        nodes.push(argument())
      } else {
        text += char
        at++
      }
    }
    flush()
    return nodes
  }

  const argument = (): Node => {
    const head = /^\s*(\w+)\s*(?:,\s*(\w+)\s*)?/.exec(source.slice(at))!
    at += head[0].length
    const [, name, type] = head
    if (!type || type === 'number') {
      at++
      return type ? { kind: 'number', name } : { kind: 'arg', name }
    }
    at++
    const arms: Record<string, Node[]> = {}
    for (;;) {
      const key = /^\s*([^\s{}]+)\s*\{/.exec(source.slice(at))
      if (!key) break
      at += key[0].length
      arms[key[1]] = sequence('}')
      at++
    }
    at += /^\s*\}/.exec(source.slice(at))![0].length
    return { kind: 'choice', name, plural: type === 'plural', arms }
  }

  return sequence(undefined)
}

const parsed = new Map<string, Node[]>()
const numberFormat = new Intl.NumberFormat(LOCALE)
const pluralRules = new Intl.PluralRules(LOCALE)

function render(nodes: Node[], values: Record<string, FormatValue>, count: string | undefined): unknown[] {
  const parts: unknown[] = []
  for (const node of nodes) {
    if (typeof node === 'string') {
      parts.push(count === undefined ? node : node.replaceAll('#', count))
    } else if (node.kind === 'arg') {
      parts.push(values[node.name] ?? `{${node.name}}`)
    } else if (node.kind === 'number') {
      parts.push(numberFormat.format(Number(values[node.name])))
    } else if (node.kind === 'choice') {
      const value = values[node.name]
      const arm = node.plural
        ? (node.arms[`=${value}`] ?? node.arms[pluralRules.select(Number(value))])
        : node.arms[String(value)]
      const inner = node.plural ? numberFormat.format(Number(value)) : count
      parts.push(...render(arm ?? node.arms.other ?? [], values, inner))
    } else {
      const children = render(node.children, values, count)
      const wrap = values[node.name]
      if (typeof wrap === 'function') parts.push(wrap(children as never[]))
      else parts.push(...children)
    }
  }
  return parts
}

/** The message's parts with its values filled in: strings, plus whatever the tag functions returned. */
export function formatParts(message: string, values: Record<string, FormatValue> = {}): unknown[] {
  let nodes = parsed.get(message)
  if (!nodes) parsed.set(message, (nodes = parse(message)))
  return render(nodes, values, undefined)
}
