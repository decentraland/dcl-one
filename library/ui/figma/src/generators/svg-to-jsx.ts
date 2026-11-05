import { optimize } from 'svgo'
import { parseSvg, type SvgNode } from '../capture/svg-tree.js'

type OptimizedIcon = {
  viewBox: string
  jsx: string
  monochrome: boolean
}

const ATTRIBUTE = /([\w:.-]+)="([^"]*)"/g
const ENTITIES: Record<string, string> = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'" }
const PAINT_ATTRIBUTES = new Set(['fill', 'stroke'])

function decodeEntities(value: string): string {
  return value.replace(/&(#\d+|#x[0-9a-f]+|\w+);/gi, (whole, entity: string) => {
    if (entity.startsWith('#x')) return String.fromCodePoint(parseInt(entity.slice(2), 16))
    if (entity.startsWith('#')) return String.fromCodePoint(Number(entity.slice(1)))
    return ENTITIES[entity] ?? whole
  })
}

function toJsxAttributeName(name: string): string {
  if (name === 'class') return 'className'
  if (name.startsWith('data-') || name.startsWith('aria-')) return name
  return name.replace(/[:-](\w)/g, (_, letter: string) => letter.toUpperCase())
}

function parseAttributes(node: SvgNode): [string, string][] {
  return [...node.attrs.matchAll(ATTRIBUTE)].map((match) => [match[1]!, decodeEntities(match[2]!)])
}

function collectPaints(node: SvgNode, colors: Set<string>, state: { dynamic: boolean }) {
  for (const [name, value] of parseAttributes(node)) {
    if (PAINT_ATTRIBUTES.has(name)) {
      if (value.startsWith('url(')) state.dynamic = true
      else if (value !== 'none') colors.add(value.toLowerCase())
    }
    if (name === 'stop-color' || name === 'style') state.dynamic = true
  }
  if (node.tag === 'linearGradient' || node.tag === 'radialGradient' || node.tag === 'image') {
    state.dynamic = true
  }
  node.children.forEach((child) => collectPaints(child, colors, state))
}

function renderStyle(value: string): string {
  const declarations = value
    .split(';')
    .map((declaration) => declaration.split(':').map((part) => part.trim()))
    .filter(([property, rule]) => property && rule)
    .map(([property, rule]) => `${toJsxAttributeName(property!)}: ${JSON.stringify(rule)}`)
  return `style={{ ${declarations.join(', ')} }}`
}

function renderAttribute(name: string, value: string): string {
  if (name === 'style') return renderStyle(value)
  const jsxName = toJsxAttributeName(name)
  return /["&{}\\\n]/.test(value)
    ? `${jsxName}={${JSON.stringify(value)}}`
    : `${jsxName}="${value}"`
}

function renderNode(node: SvgNode, depth: number, monochrome: boolean): string {
  const pad = '  '.repeat(depth)
  const attributes = parseAttributes(node).map(([name, value]) => {
    const paint = monochrome && PAINT_ATTRIBUTES.has(name) && value !== 'none'
    return renderAttribute(name, paint ? 'currentColor' : value)
  })
  const open = `${pad}<${node.tag}${attributes.map((attribute) => ` ${attribute}`).join('')}`
  if (node.children.length === 0) return `${open} />`
  const children = node.children.map((child) => renderNode(child, depth + 1, monochrome))
  return `${open}>\n${children.join('\n')}\n${pad}</${node.tag}>`
}

function optimizeIcon(svg: string, prefix: string, depth: number): OptimizedIcon {
  const { data } = optimize(svg, {
    multipass: true,
    plugins: ['preset-default', { name: 'prefixIds', params: { prefix, delim: '-' } }]
  })
  const root = parseSvg(data)
  const viewBox = parseAttributes(root).find(([name]) => name === 'viewBox')?.[1]
  if (!viewBox) throw new Error(`Icon ${prefix} has no viewBox`)

  const colors = new Set<string>()
  const state = { dynamic: false }
  root.children.forEach((child) => collectPaints(child, colors, state))
  const monochrome = !state.dynamic && colors.size <= 1

  return {
    viewBox,
    monochrome,
    jsx: root.children.map((child) => renderNode(child, depth, monochrome)).join('\n')
  }
}

export { optimizeIcon }
export type { OptimizedIcon }
