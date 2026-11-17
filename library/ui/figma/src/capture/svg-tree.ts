type SvgNode = {
  tag: string
  attrs: string
  children: SvgNode[]
  selfClosing: boolean
  text: string
}

const TAG_PATTERN = /<(\/?)([a-zA-Z][\w:-]*)((?:[^>"']|"[^"]*"|'[^']*')*?)(\/?)>/g

function parseSvg(source: string): SvgNode {
  const root: SvgNode = { tag: '#root', attrs: '', children: [], selfClosing: false, text: '' }
  const stack: SvgNode[] = [root]
  let cursor = 0

  for (const match of source.matchAll(TAG_PATTERN)) {
    const [whole, closing, tag, attrs, selfClosing] = match
    const current = stack[stack.length - 1]!
    const between = source.slice(cursor, match.index).trim()
    if (between) current.text += between
    cursor = match.index + whole.length

    if (closing) {
      stack.pop()
      continue
    }
    const node: SvgNode = {
      tag: tag!,
      attrs: attrs!,
      children: [],
      selfClosing: selfClosing === '/',
      text: ''
    }
    current.children.push(node)
    if (!node.selfClosing) stack.push(node)
  }

  return root.children[0] ?? root
}

function serialize(node: SvgNode): string {
  if (node.selfClosing) return `<${node.tag}${node.attrs}/>`
  const inner = node.children.map(serialize).join('\n')
  return `<${node.tag}${node.attrs}>${node.text}${inner ? `\n${inner}\n` : ''}</${node.tag}>`
}

function getId(node: SvgNode): string | undefined {
  return /\sid="([^"]*)"/.exec(node.attrs)?.[1]
}

function collectIds(node: SvgNode, into: Map<string, SvgNode> = new Map()): Map<string, SvgNode> {
  const id = getId(node)
  if (id) into.set(id, node)
  for (const child of node.children) collectIds(child, into)
  return into
}

function referencedIds(markup: string): string[] {
  const ids = new Set<string>()
  for (const match of markup.matchAll(/url\(#([^)]+)\)|href="#([^"]+)"/g)) {
    ids.add((match[1] ?? match[2])!)
  }
  return [...ids]
}

export { collectIds, getId, parseSvg, referencedIds, serialize }
export type { SvgNode }
