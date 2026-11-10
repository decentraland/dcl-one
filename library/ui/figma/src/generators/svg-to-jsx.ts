import { optimize } from 'svgo'
import { parseSvg, type SvgNode } from '../capture/svg-tree.js'

type OptimizedIcon = {
  viewBox: string
  monochrome: boolean
  // True when the SVG declares ids, so the component must scope them per rendered instance.
  scopedIds: boolean
  jsx: (depth: number) => string
}

// The local variable a generated component holds its per-instance id prefix in.
const ID_SCOPE = 'svgId'

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

type RenderContext = { monochrome: boolean; idPrefix: string; inMask: boolean }

// svgo's prefixIds writes every id as `<prefix>-<id>` and every reference as `#<prefix>-<id>`, so both
// become a template literal on the instance prefix. Returns null when the value holds no id.
function scopedIdExpression(name: string, value: string, idPrefix: string): string | null {
  const marker = `${idPrefix}-`
  const declares = name === 'id' && value.startsWith(marker)
  if (!declares && !value.includes(`#${marker}`)) return null
  const escaped = value.replace(/[\\`]|\$\{/g, (match) => `\\${match}`)
  const scoped = escaped.split(`#${marker}`).join(`#\${${ID_SCOPE}}-`)
  return `\`${declares ? `\${${ID_SCOPE}}-${scoped.slice(marker.length)}` : scoped}\``
}

function renderStyle(value: string, idPrefix: string): string {
  const declarations = value
    .split(';')
    .map((declaration) => declaration.split(':').map((part) => part.trim()))
    .filter(([property, rule]) => property && rule)
    .map(
      ([property, rule]) =>
        `${toJsxAttributeName(property!)}: ${scopedIdExpression(property!, rule!, idPrefix) ?? JSON.stringify(rule)}`
    )
  return `style={{ ${declarations.join(', ')} }}`
}

function renderAttribute(name: string, value: string, idPrefix: string): string {
  if (name === 'style') return renderStyle(value, idPrefix)
  const jsxName = toJsxAttributeName(name)
  const scoped = scopedIdExpression(name, value, idPrefix)
  if (scoped) return `${jsxName}={${scoped}}`
  return /["&{}\\\n]/.test(value)
    ? `${jsxName}={${JSON.stringify(value)}}`
    : `${jsxName}="${value}"`
}

// A mask works by luminance: its paints are data, not color, so they keep their original value even
// in a monochrome icon. Only paint outside every mask follows currentColor.
function renderNode(node: SvgNode, depth: number, context: RenderContext): string {
  const pad = '  '.repeat(depth)
  const scope = node.tag === 'mask' ? { ...context, inMask: true } : context
  const attributes = parseAttributes(node).map(([name, value]) => {
    const paint =
      scope.monochrome && !scope.inMask && PAINT_ATTRIBUTES.has(name) && value !== 'none'
    return renderAttribute(name, paint ? 'currentColor' : value, scope.idPrefix)
  })
  const open = `${pad}<${node.tag}${attributes.map((attribute) => ` ${attribute}`).join('')}`
  if (node.children.length === 0) return `${open} />`
  const children = node.children.map((child) => renderNode(child, depth + 1, scope))
  return `${open}>\n${children.join('\n')}\n${pad}</${node.tag}>`
}

function declaresIds(node: SvgNode, idPrefix: string): boolean {
  return (
    parseAttributes(node).some(
      ([name, value]) => name === 'id' && value.startsWith(`${idPrefix}-`)
    ) || node.children.some((child) => declaresIds(child, idPrefix))
  )
}

function optimizeIcon(svg: string, prefix: string): OptimizedIcon {
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

  const context: RenderContext = { monochrome, idPrefix: prefix, inMask: false }
  return {
    viewBox,
    monochrome,
    scopedIds: root.children.some((child) => declaresIds(child, prefix)),
    jsx: (depth) => root.children.map((child) => renderNode(child, depth, context)).join('\n')
  }
}

export { ID_SCOPE, optimizeIcon }
export type { OptimizedIcon }
