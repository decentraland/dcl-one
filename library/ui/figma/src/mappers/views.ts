import type { CssBlock, StateRule, ViewNode } from '../ir/components.js'
import type { RawNode, RawPaint } from '../ir/raw-components.js'
import type { TextStyle } from '../ir/schemas.js'
import type { ColorResolver, PaintLike } from './colors.js'

type NumberTokens = { space: Map<number, string>; radius: Map<number, string> }

type RefTarget = { name: string; props: Record<string, string | boolean> }

type Report = { notes: Set<string>; manual: Set<string> }

type ViewEnv = {
  colors: ColorResolver
  textStyles: Map<string, TextStyle>
  textStyleAliases: Record<string, string>
  numbers: NumberTokens
  icons: Map<string, string>
  iconAliases: Record<string, string>
  refsById: Map<string, RefTarget>
  report: Report
  // Image fill bytes by Figma image hash, from the component's capture.
  images?: Record<string, string>
}

type Parent = { node: RawNode; positioned: boolean } | undefined

const r2 = (value: number) => Math.round(value * 100) / 100
const px = (value: number) => `${r2(value)}px`
const WEIGHTS: Record<string, number> = { Regular: 400, Medium: 500, 'Semi Bold': 600, Bold: 700 }
const JUSTIFY: Record<string, string> = {
  MIN: 'flex-start',
  CENTER: 'center',
  MAX: 'flex-end',
  SPACE_BETWEEN: 'space-between'
}
const ALIGN: Record<string, string> = {
  MIN: 'flex-start',
  CENTER: 'center',
  MAX: 'flex-end',
  BASELINE: 'baseline'
}
const BOX_VECTOR_NAME = /^(rectangle|bg|container)/i
const ARBITRARY_GLYPHS = new Set(['STAR', 'POLYGON', 'BOOLEAN_OPERATION', 'LINE', 'DRAWING'])

function lengthValue(value: number, tokens: Map<number, string>): string {
  const token = tokens.get(r2(value))
  return token ? `var(${token})` : px(value)
}

function radiusDecl(node: RawNode, env: ViewEnv): [string, string] | undefined {
  if (!node.radius) return undefined
  if (Array.isArray(node.radius)) {
    return ['border-radius', node.radius.map((value) => px(value)).join(' ')]
  }
  const half = Math.min(node.w, node.h) / 2
  const pill = node.radius >= 100 || (half > 0 && node.radius >= half - 0.5)
  if (pill) return ['border-radius', 'var(--dcl-radius-full)']
  return ['border-radius', lengthValue(node.radius, env.numbers.radius)]
}

// A paint without a colour is reported, never drawn as a default colour.
function solidCss(paint: PaintLike, env: ViewEnv, label: string): string | undefined {
  const resolved = env.colors.resolve(paint)
  if (resolved.via !== 'unresolved') return resolved.css
  env.report.manual.add(`unresolved-paint:${label}: the paint has no colour CSS can draw`)
  return undefined
}

// CSS form of a Figma linear gradient over a w x h box, so its stops can use colour variables (an SVG
// data URL cannot). Figma's transform maps the box (0..1 on both axes) to the gradient, whose position t is
// the first row; CSS measures stops along a line through the centre whose length depends on the angle.
function linearGradientCss(
  paint: RawPaint,
  w: number,
  h: number,
  env: ViewEnv
): string | undefined {
  const [a = 1, b = 0, c = 0] = paint.transform?.[0] ?? []
  if (!paint.transform || !paint.stops?.length || w <= 0 || h <= 0) return undefined
  const [dx, dy] = [a / w, b / h]
  const slope = Math.hypot(dx, dy)
  if (slope < 1e-12) return undefined
  const angle = Math.atan2(dx, -dy)
  const length = Math.abs(w * Math.sin(angle)) + Math.abs(h * Math.cos(angle))
  const centre = a / 2 + b / 2 + c
  const stops = paint.stops.map((stop) => {
    const color = env.colors.resolve({
      token: stop.token,
      hex: stop.hex,
      opacity: stop.alpha * (paint.opacity ?? 1)
    }).css
    return `${color} ${r2((0.5 + (stop.position - centre) / (length * slope)) * 100)}%`
  })
  return `linear-gradient(${r2(((angle * 180) / Math.PI + 360) % 360)}deg, ${stops.join(', ')})`
}

function strokeWeightsOf(node: RawNode): [number, number, number, number] | undefined {
  if (node.strokeWeights) return node.strokeWeights
  if (typeof node.strokeWeight === 'number')
    return [node.strokeWeight, node.strokeWeight, node.strokeWeight, node.strokeWeight]
  return undefined
}

// A box-shadow cannot paint a gradient, so a gradient stroke is drawn by an ::after overlay: a transparent
// border of the stroke width carries the gradient and the mask cuts the padding box out, leaving a ring
// that follows border-radius. Borders snap to device pixels on every side; a padding-based content box
// rounded sub-pixel strokes away on the right and bottom edges.
// It is drawn after the children, as Figma paints strokes over them, and never takes layout space.
function ringDecls(node: RawNode, env: ViewEnv): [string, string][] | undefined {
  const stroke = node.strokes?.[0]
  if (!stroke || stroke.type === 'SOLID') return undefined
  const label = node.name ?? node.type
  const weights = strokeWeightsOf(node)
  const background =
    stroke.type === 'GRADIENT_LINEAR' && weights
      ? linearGradientCss(stroke, node.w, node.h, env)
      : undefined
  if (!weights || !background) {
    env.report.manual.add(`unsupported-stroke-paint:${label}: a ${stroke.type} stroke is not drawn`)
    return undefined
  }
  const outset = node.strokeAlign === 'OUTSIDE' ? 1 : node.strokeAlign === 'CENTER' ? 0.5 : 0
  if (outset) env.report.notes.add(`stroke-ring-radius-approximated:${label}`)
  const mask = 'linear-gradient(#000 0 0) padding-box, linear-gradient(#000 0 0)'
  return [
    ['content', "''"],
    ['position', 'absolute'],
    ['inset', outset ? weights.map((weight) => px(-weight * outset)).join(' ') : '0'],
    ['border-style', 'solid'],
    ['border-color', 'transparent'],
    ['border-width', weights.map((weight) => px(weight)).join(' ')],
    ['border-radius', 'inherit'],
    ['background', `${background} border-box`],
    ['-webkit-mask', mask],
    ['-webkit-mask-composite', 'xor'],
    ['mask', mask],
    ['mask-composite', 'exclude'],
    ['pointer-events', 'none'],
    ['opacity', '1']
  ]
}

// Figma's strokesIncludedInLayout makes an inside stroke take layout space, so content starts after
// padding plus stroke. Only an explicit true counts: a capture without the field keeps padding alone.
function strokeLayoutInset(node: RawNode, env: ViewEnv): [number, number, number, number] {
  const weights = strokeWeightsOf(node)
  if (node.strokesIncludedInLayout !== true || !node.strokes?.length || !weights)
    return [0, 0, 0, 0]
  if (node.strokeAlign !== undefined && node.strokeAlign !== 'INSIDE') {
    env.report.manual.add(
      `unsupported-stroke-layout:${node.name ?? node.type}: a ${node.strokeAlign} stroke included in layout`
    )
    return [0, 0, 0, 0]
  }
  return weights
}

function paintCss(paint: RawPaint, env: ViewEnv): string {
  if (paint.type === 'SOLID') return solidCss(paint, env, 'fill') ?? 'transparent'
  if (paint.type.startsWith('GRADIENT') && paint.stops) {
    const effectiveStops = paint.stops.map((stop) => ({
      ...stop,
      alpha: stop.alpha * (paint.opacity ?? 1)
    }))
    const stops = effectiveStops
      .map(
        (stop) =>
          `${env.colors.resolve({ hex: stop.hex, opacity: stop.alpha }).css} ${r2(stop.position * 100)}%`
      )
      .join(', ')
    const transform = paint.transform
    if (transform && ['GRADIENT_LINEAR', 'GRADIENT_RADIAL'].includes(paint.type)) {
      const [a = 1, b = 0, c = 0] = transform[0] ?? []
      const [d = 0, e = 1, f = 0] = transform[1] ?? []
      const determinant = a * e - b * d
      if (Math.abs(determinant) > 1e-12) {
        const matrix = [e, -d, -b, a, b * f - e * c, d * c - a * f].map(
          (value) => value / determinant
        )
        const tag = paint.type === 'GRADIENT_LINEAR' ? 'linearGradient' : 'radialGradient'
        const points =
          tag === 'linearGradient' ? 'x1="0" y1="0.5" x2="1" y2="0.5"' : 'cx="0.5" cy="0.5" r="0.5"'
        const svgStops = effectiveStops
          .map(
            (stop) =>
              `<stop offset="${stop.position}" stop-color="${stop.hex}" stop-opacity="${stop.alpha}"/>`
          )
          .join('')
        const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1" preserveAspectRatio="none"><defs><${tag} id="g" gradientUnits="userSpaceOnUse" ${points} gradientTransform="matrix(${matrix.join(' ')})">${svgStops}</${tag}></defs><rect width="1" height="1" fill="url(#g)"/></svg>`
        return `url("data:image/svg+xml,${encodeURIComponent(svg)}")`
      }
    }
    env.report.manual.add(
      'missing-gradient-transform: gradient geometry could not be reconstructed'
    )
    return `linear-gradient(180deg, ${stops})`
  }
  env.report.notes.add('image-fill-replaced-by-placeholder-colour')
  env.report.manual.add('missing-image-asset: the source image was not captured')
  return 'var(--dcl-neutrals-graffito)'
}

const ASPECT: Record<string, string> = { FILL: 'slice', CROP: 'slice', FIT: 'meet' }

// The photo is wrapped in an SVG sized to the node, so the generator externalizes and validates it
// like any other artwork. FILL covers the box and FIT fits inside it, as Figma scales the image.
function imageLayer(paint: RawPaint, node: RawNode, env: ViewEnv): string | undefined {
  const src = paint.imageRef ? env.images?.[paint.imageRef] : undefined
  if (!src) return undefined
  const aspect = ASPECT[paint.scaleMode ?? 'FILL']
  if (!aspect)
    env.report.notes.add(`image-scale-mode-approximated: ${paint.scaleMode} drawn as FILL`)
  if (paint.scaleMode === 'CROP') env.report.notes.add('image-crop-approximated: drawn as FILL')
  if ((paint.opacity ?? 1) < 1)
    env.report.notes.add('image-fill-opacity-ignored: the image is drawn fully opaque')
  const [w, h] = [r2(node.w), r2(node.h)]
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${w} ${h}" preserveAspectRatio="none"><image width="${w}" height="${h}" preserveAspectRatio="xMidYMid ${aspect ?? 'slice'}" href="${src}"/></svg>`
  return `url("data:image/svg+xml,${encodeURIComponent(svg)}") 0 0 / 100% 100% no-repeat`
}

function fillDecls(node: RawNode, env: ViewEnv): [string, string][] {
  if (!Array.isArray(node.fills) || node.fills.length === 0) return []
  const backgrounds = [...node.fills].reverse().map((paint) => {
    const image = paint.type === 'IMAGE' ? imageLayer(paint, node, env) : undefined
    if (image) return image
    const background = paintCss(paint, env)
    return background.startsWith('url(') ? `${background} 0 0 / 100% 100% no-repeat` : background
  })
  return [['background', backgrounds.join(', ')]]
}

function textPaintDecls(fills: RawNode['fills'], env: ViewEnv): [string, string][] {
  const paint = Array.isArray(fills) ? fills[0] : undefined
  if (!paint) return []
  if (paint.type === 'SOLID') {
    const color = solidCss(paint, env, 'text')
    return color ? [['color', color]] : []
  }
  if (paint.type.startsWith('GRADIENT'))
    return [
      ['background-image', paintCss(paint, env)],
      ['background-size', '100% 100%'],
      ['background-repeat', 'no-repeat'],
      ['background-clip', 'text'],
      ['-webkit-background-clip', 'text'],
      ['color', 'transparent']
    ]
  env.report.manual.add('unsupported-text-paint: text paint is neither solid nor gradient')
  return []
}

function shadowDecls(node: RawNode, env: ViewEnv): [string, string][] {
  const shadows: string[] = []
  // Other paints are drawn by ringDecls or reported there; resolving them as a colour drew them black.
  const stroke = node.strokes?.[0]?.type === 'SOLID' ? node.strokes[0] : undefined
  const color = stroke ? solidCss(stroke, env, 'stroke') : undefined
  if (stroke && color && node.strokeWeights && node.strokeAlign === 'INSIDE') {
    const [top, right, bottom, left] = node.strokeWeights
    if (top) shadows.push(`inset 0 ${px(top)} 0 0 ${color}`)
    if (right) shadows.push(`inset ${px(-right)} 0 0 0 ${color}`)
    if (bottom) shadows.push(`inset 0 ${px(-bottom)} 0 0 ${color}`)
    if (left) shadows.push(`inset ${px(left)} 0 0 0 ${color}`)
  } else if (stroke && color && typeof node.strokeWeight === 'number') {
    const inset = node.strokeAlign === 'OUTSIDE' ? '' : 'inset '
    shadows.push(`${inset}0 0 0 ${px(node.strokeWeight)} ${color}`)
  } else if (stroke && color) {
    env.report.notes.add('mixed-stroke-weight-approximated')
    env.report.manual.add(
      'missing-stroke-weights: individual border sides were not captured or are unsupported'
    )
    shadows.push(`inset 0 0 0 1px ${color}`)
  }
  for (const effect of node.effects ?? []) {
    if (effect.type !== 'DROP_SHADOW' && effect.type !== 'INNER_SHADOW') continue
    const color = env.colors.resolve({ hex: effect.color ?? '#000000', opacity: effect.alpha }).css
    const [x, y] = effect.offset ?? [0, 0]
    const inset = effect.type === 'INNER_SHADOW' ? 'inset ' : ''
    shadows.push(
      `${inset}${px(x)} ${px(y)} ${px(effect.radius)} ${px(effect.spread ?? 0)} ${color}`
    )
  }
  return shadows.length > 0 ? [['box-shadow', shadows.join(', ')]] : []
}

function sizingDecls(node: RawNode, parent: Parent, isRoot: boolean): [string, string][] {
  const decls: [string, string][] = []
  const fixedWidth: [string, string] = ['width', px(node.w)]
  const fixedHeight: [string, string] = ['height', px(node.h)]

  if (isRoot) {
    const [horizontal, vertical] = node.sizing ?? ['FIXED', 'FIXED']
    if (horizontal === 'FILL') decls.push(['width', '100%'])
    else if (horizontal !== 'HUG') decls.push(fixedWidth)
    if (vertical === 'FILL') decls.push(['height', '100%'])
    else if (vertical !== 'HUG') decls.push(fixedHeight)
    return decls
  }

  const parentMode = parent?.node.layout?.mode
  if (!parentMode) {
    decls.push(fixedWidth, fixedHeight, ['flex', 'none'])
    return decls
  }
  const [horizontal, vertical] = node.sizing ?? ['FIXED', 'FIXED']
  const mainIsHorizontal = parentMode === 'HORIZONTAL'
  const axes: [string, string, string, string, boolean][] = [
    ['width', px(node.w), horizontal, 'width', mainIsHorizontal],
    ['height', px(node.h), vertical, 'height', !mainIsHorizontal]
  ]
  for (const [property, size, sizing, , isMain] of axes) {
    if (sizing === 'FIXED') {
      decls.push([property, size])
      if (isMain) decls.push(['flex-shrink', '0'])
    } else if (sizing === 'HUG' && isMain && node.type !== 'TEXT') {
      decls.push(['flex-shrink', '0'])
    } else if (sizing === 'FILL') {
      if (isMain)
        decls.push(['flex', '1 1 0'], [property === 'width' ? 'min-width' : 'min-height', '0'])
      else decls.push(['align-self', 'stretch'])
    }
  }
  return decls
}

// CSS paints positioned elements over in-flow siblings, while Figma paints in layer order. Positioning
// the in-flow layers too keeps an absolute layer drawn earlier (a scrim, a backdrop) underneath them.
const sharesParentWithAbsolute = (parent: Parent) =>
  !!parent?.node.layout &&
  (parent.node.children ?? []).some((child) => child.absolute && !child.hidden)

// Figma's negative item spacing overlaps each in-flow child onto the previous one. CSS has no negative gap, so
// every child after the first moves back by a negative main-axis margin, which also gives the hug size Figma
// computes. Overlapping children paint in their captured layer order, later ones on top.
function overlapDecls(node: RawNode, parent: Parent, env: ViewEnv): [string, string][] {
  const layout = parent?.node.layout
  if (!layout || layout.gap >= 0 || layout.wrap || node.absolute) return []
  const flow = (parent.node.children ?? []).filter(
    (child) => !child.absolute && !child.hidden && !child.isMask
  )
  const index = flow.indexOf(node)
  if (index < 0)
    throw new Error(`Overlapping child is not among its siblings: ${node.name ?? node.type}`)
  const magnitude = lengthValue(-layout.gap, env.numbers.space)
  const margin: [string, string][] =
    index > 0
      ? [
          [
            layout.mode === 'VERTICAL' ? 'margin-top' : 'margin-left',
            magnitude.startsWith('var(') ? `calc(${magnitude} * -1)` : px(layout.gap)
          ]
        ]
      : []
  return [['position', 'relative'], ...margin]
}

function offsetDecls(node: RawNode, parent: Parent, env: ViewEnv): [string, string][] {
  if (!parent?.positioned && !node.absolute) {
    const overlap = overlapDecls(node, parent, env)
    if (overlap.length) return overlap
    return sharesParentWithAbsolute(parent) ? [['position', 'relative']] : []
  }
  if (node.x !== undefined && node.y !== undefined) {
    return [
      ['position', 'absolute'],
      ['left', px(node.x)],
      ['top', px(node.y)]
    ]
  }
  if (!parent?.positioned) return []
  const coversParent =
    Math.abs(node.w - parent.node.w) <= 1 && Math.abs(node.h - parent.node.h) <= 1
  if (coversParent)
    return [
      ['position', 'absolute'],
      ['inset', '0']
    ]
  env.report.notes.add('absolute-children-centered-approximation')
  return [
    ['position', 'absolute'],
    ['top', '50%'],
    ['left', '50%'],
    ['transform', 'translate(-50%, -50%)']
  ]
}

function containerDecls(node: RawNode, env: ViewEnv): [string, string][] {
  const layout = node.layout
  if (!layout)
    return [
      ['position', 'relative'],
      ...(node.clipsContent ? [['overflow', 'hidden'] as [string, string]] : [])
    ]
  const decls: [string, string][] = [
    ['display', 'flex'],
    ['flex-direction', layout.mode === 'VERTICAL' ? 'column' : 'row'],
    ['justify-content', JUSTIFY[layout.main] ?? 'flex-start'],
    ['align-items', ALIGN[layout.cross] ?? 'flex-start']
  ]
  if (node.clipsContent) decls.push(['overflow', 'hidden'])
  if (layout.wrap) decls.push(['flex-wrap', 'wrap'])
  if (layout.gap > 0) decls.push(['gap', lengthValue(layout.gap, env.numbers.space)])
  // A negative gap overlaps the children through their margins (overlapDecls); wrapped rows stay unsupported.
  else if (layout.gap < 0 && layout.wrap) env.report.notes.add('negative-gap-ignored')
  const inset = strokeLayoutInset(node, env)
  const [top = 0, right = 0, bottom = 0, left = 0] = layout.pad.map(
    (value, side) => value + (inset[side] ?? 0)
  )
  if (top || right || bottom || left) {
    decls.push([
      'padding',
      [top, right, bottom, left].map((value) => lengthValue(value, env.numbers.space)).join(' ')
    ])
  }
  return decls
}

function resolveTextStyle(
  node: RawNode,
  env: ViewEnv
): { textStyle?: string; decls: [string, string][] } {
  const decls: [string, string][] = []
  const textCases: Record<string, string> = {
    UPPER: 'uppercase',
    LOWER: 'lowercase',
    TITLE: 'capitalize',
    ORIGINAL: 'none'
  }
  const textCase = textCases[node.textCase ?? '']
  if (textCase) decls.push(['text-transform', textCase])
  if (node.letterSpacing)
    decls.push([
      'letter-spacing',
      node.letterSpacing.unit === 'PERCENT'
        ? `${node.letterSpacing.value / 100}em`
        : px(node.letterSpacing.value)
    ])
  if (node.textAlignHorizontal) decls.push(['text-align', node.textAlignHorizontal.toLowerCase()])
  if (node.lineHeight)
    decls.push([
      'line-height',
      node.lineHeight.unit === 'AUTO'
        ? 'normal'
        : node.lineHeight.unit === 'PERCENT'
          ? String((node.lineHeight.value ?? 120) / 100)
          : px(node.lineHeight.value ?? 0)
    ])
  const named = node.textStyle
    ? (env.textStyleAliases[node.textStyle] ?? node.textStyle)
    : undefined
  if (named && env.textStyles.has(named)) {
    if (node.textStyle !== named)
      env.report.notes.add(`legacy-text-style:${node.textStyle}->${named}`)
    if (node.font) {
      const [, ...weight] = node.font.split(' ')
      decls.push(['font-weight', String(WEIGHTS[weight.join(' ')] ?? 400)])
    }
    if (node.fontSize !== undefined) decls.push(['font-size', px(node.fontSize)])
    return { textStyle: named, decls }
  }
  if (named) {
    env.report.notes.add(`unknown-text-style:${named}`)
    if (!node.font || node.fontSize === undefined) return { textStyle: 'body/md', decls }
  }
  const [, ...weightWords] = (node.font ?? 'Inter Regular').split(' ')
  const weightName = weightWords.join(' ')
  const size = node.fontSize ?? 16
  const upper = node.textCase === 'UPPER'
  for (const style of env.textStyles.values()) {
    if (
      style.fontSize === size &&
      style.fontStyle === weightName &&
      (style.textCase === 'UPPER') === upper
    ) {
      if (node.textCase === 'TITLE') decls.push(['text-transform', 'capitalize'])
      return { textStyle: style.name, decls }
    }
  }
  env.report.notes.add(`text-without-style:${weightName || 'Regular'} ${size}px`)
  decls.push(
    ['font-family', "'Inter', sans-serif"],
    ['font-weight', String(WEIGHTS[weightName] ?? 400)],
    ['font-size', px(size)],
    ['line-height', '1.25'],
    ['text-transform', upper ? 'uppercase' : node.textCase === 'TITLE' ? 'capitalize' : 'none']
  )
  return { decls }
}

function slotName(kind: 'text' | 'icon', index: number): string {
  const base = kind === 'text' ? 'label' : 'icon'
  return index === 0 ? base : `${base}${index + 1}`
}

type BuildState = { text: number; icon: number }

function positionDecls(node: RawNode, parent: Parent, env: ViewEnv): [string, string][] {
  const decls = offsetDecls(node, parent, env)
  if (parent?.node.layout && parent.node.layout.gap < 0 && !parent.node.layout.wrap) {
    const layers = (parent.node.children ?? []).filter((child) => !child.hidden && !child.isMask)
    const index = layers.indexOf(node)
    if (index >= 0) decls.push(['z-index', String(index + 1)])
  }
  if (!node.flip?.some(Boolean)) return decls
  const translation = decls.find(([key]) => key === 'transform')?.[1]
  return [
    ...decls.filter(([key]) => key !== 'transform'),
    [
      'transform',
      `${translation ? translation + ' ' : ''}scale(${node.flip[0] ? -1 : 1}, ${node.flip[1] ? -1 : 1})`
    ],
    ['transform-origin', 'center']
  ]
}

function boxCss(node: RawNode, parent: Parent, isRoot: boolean, env: ViewEnv): CssBlock {
  const decls: [string, string][] = [['box-sizing', 'border-box'], ...containerDecls(node, env)]
  if (isRoot) decls.unshift(['display', 'inline-flex'])
  const position = positionDecls(node, parent, env)
  if (
    node.layout &&
    node.children?.some((child) => child.absolute && !child.hidden) &&
    !position.some(([key]) => key === 'position')
  ) {
    decls.push(['position', 'relative'])
  }
  decls.push(...sizingDecls(node, parent, isRoot), ...position)
  const radius = radiusDecl(node, env)
  if (radius) decls.push(radius)
  decls.push(...fillDecls(node, env), ...shadowDecls(node, env))
  if (node.opacity !== undefined) decls.push(['opacity', String(node.opacity)])
  return withRing(decls, node, env)
}

// The ring overlay is positioned against its own box, so a box that is not positioned yet becomes relative.
function withRing(decls: [string, string][], node: RawNode, env: ViewEnv): CssBlock {
  const merged = new Map<string, string>()
  for (const [property, value] of decls) merged.set(property, value)
  const after = ringDecls(node, env)
  if (!after) return { decls: [...merged.entries()] }
  if (!merged.has('position')) merged.set('position', 'relative')
  return { decls: [...merged.entries()], after }
}

// `source` is the captured node itself when `node` is a trimmed copy, so its place among its siblings is known.
function leafBoxCss(
  node: RawNode,
  parent: Parent,
  env: ViewEnv,
  circle: boolean,
  source: RawNode = node
): CssBlock {
  const decls: [string, string][] = [
    ['box-sizing', 'border-box'],
    ...sizingDecls(node, parent, false),
    ...positionDecls(source, parent, env)
  ]
  if (node.layout && !node.children?.length) {
    if (node.sizing?.[0] === 'HUG') decls.push(['width', px(node.w)])
    if (node.sizing?.[1] === 'HUG') decls.push(['height', px(node.h)])
  }
  if (circle) decls.push(['border-radius', '50%'])
  else {
    const radius = radiusDecl(node, env)
    if (radius) decls.push(radius)
  }
  decls.push(...fillDecls(node, env), ...shadowDecls(node, env))
  if (node.opacity !== undefined) decls.push(['opacity', String(node.opacity)])
  return withRing(decls, node, env)
}

// Encoded by hand rather than base64 so the generated CSS stays readable and diffable. The characters that
// stay raw are safe inside a double-quoted url() and inside a template literal.
function svgBackground(svg: string): string {
  const compact = svg.replace(/\s+/g, ' ').replace(/"/g, "'").trim()
  const encoded = encodeURIComponent(compact)
    .replace(/%20/g, ' ')
    .replace(/%27/g, "'")
    .replace(/%3D/g, '=')
    .replace(/%2F/g, '/')
    .replace(/%3A/g, ':')
    .replace(/%2C/g, ',')
  return `url("data:image/svg+xml,${encoded}")`
}

function maskBackground(mask: RawNode, env: ViewEnv): string | undefined {
  const render = (node: RawNode): string | undefined => {
    if (node.hidden || node.opacity === 0) return ''
    if (node.type === 'TEXT' || (node.layout && node.children?.length)) return undefined
    const parts: string[] = []
    if (node.svg || node.png) {
      const href = node.svg
        ? `data:image/svg+xml,${encodeURIComponent(node.svg)}`
        : `data:image/png;base64,${node.png}`
      parts.push(`<image width="${node.w}" height="${node.h}" href="${href}"/>`)
    } else if (Array.isArray(node.fills)) {
      for (const paint of node.fills) {
        if (paint.type === 'SOLID' && paint.hex && /^#[0-9a-f]{3,8}$/i.test(paint.hex)) {
          const shape =
            node.type === 'ELLIPSE'
              ? `<ellipse cx="${node.w / 2}" cy="${node.h / 2}" rx="${node.w / 2}" ry="${node.h / 2}"`
              : `<rect width="${node.w}" height="${node.h}" rx="${typeof node.radius === 'number' ? node.radius : 0}"`
          parts.push(`${shape} fill="${paint.hex}" opacity="${paint.opacity ?? 1}"/>`)
        } else if (paint.type.startsWith('GRADIENT')) {
          const image = /^url\("([^"]+)"\)$/.exec(paintCss(paint, env))?.[1]
          if (!image) return undefined
          parts.push(
            `<image width="${node.w}" height="${node.h}" href="${image}" opacity="${paint.opacity ?? 1}"/>`
          )
        } else return undefined
      }
    }
    for (const child of node.children ?? []) {
      const childSvg = render(child)
      if (childSvg === undefined) return undefined
      parts.push(childSvg)
    }
    return `<g transform="translate(${node.x ?? 0} ${node.y ?? 0})" opacity="${node.opacity ?? 1}">${parts.join('')}</g>`
  }
  if (mask.children?.length || (Array.isArray(mask.fills) && mask.fills.length)) {
    const content = render({ ...mask, x: 0, y: 0 })
    if (content !== undefined)
      return svgBackground(
        `<svg xmlns="http://www.w3.org/2000/svg" width="${mask.w}" height="${mask.h}" viewBox="0 0 ${mask.w} ${mask.h}">${content}</svg>`
      )
  }
  return mask.mask?.svg ? svgBackground(mask.mask.svg) : undefined
}

// A rotated Figma LINE reports its length as width and 0 as height, while the exported SVG carries the box that
// is actually painted.
function paintedSize(node: RawNode): Pick<RawNode, 'w' | 'h'> {
  if (!node.svg || (node.w !== 0 && node.h !== 0)) return { w: node.w, h: node.h }
  const width = Number(/<svg[^>]*\swidth="([\d.]+)"/.exec(node.svg)?.[1])
  const height = Number(/<svg[^>]*\sheight="([\d.]+)"/.exec(node.svg)?.[1])
  return { w: width || node.w, h: height || node.h }
}

function artworkNode(
  node: RawNode,
  path: number[],
  parent: Parent,
  env: ViewEnv
): ViewNode | undefined {
  const picture = node.svg
    ? svgBackground(node.svg)
    : node.png
      ? `url("data:image/png;base64,${node.png}")`
      : undefined
  if (!picture) return undefined
  const css = leafBoxCss(
    { ...node, ...paintedSize(node), fills: undefined, strokes: undefined, effects: undefined },
    parent,
    env,
    false,
    node
  )
  css.decls.push(['background', `${picture} center / contain no-repeat`])
  return { kind: 'box', path, name: node.name ?? 'artwork', css }
}

function glyphNode(
  node: RawNode,
  path: number[],
  parent: Parent,
  env: ViewEnv,
  reason: string
): ViewNode {
  env.report.manual.add(reason)
  const tint = node.tint?.stroke ?? node.tint?.fill
  const css = leafBoxCss(
    { ...node, fills: undefined, strokes: undefined, effects: undefined },
    parent,
    env,
    false,
    node
  )
  css.decls.push(
    ['border', `1px dashed ${(tint && solidCss(tint, env, 'tint')) || 'currentColor'}`],
    ['border-radius', '4px']
  )
  return {
    kind: 'glyph',
    path,
    name: node.name ?? 'glyph',
    css,
    glyph: { name: node.name ?? 'glyph', reason }
  }
}

function instanceNode(
  node: RawNode,
  path: number[],
  parent: Parent,
  env: ViewEnv,
  state: BuildState
): ViewNode {
  const component = node.component ?? node.name ?? 'instance'
  const sizing = leafBoxCss(
    { ...node, fills: undefined, strokes: undefined, effects: undefined },
    parent,
    env,
    false,
    node
  )
  // A set whose variants are different glyphs (ContextualMenuIcons) is aliased per variant: `Set#Property=Value`.
  const iconName =
    env.iconAliases[`${component}#${node.variant ?? ''}`] ?? env.iconAliases[component] ?? component
  const iconComponent = env.icons.get(iconName)
  if (iconComponent) {
    if (iconName !== component) env.report.notes.add(`legacy-icon:${component}->${iconName}`)
    const tint = node.tint?.stroke ?? node.tint?.fill
    const props: Record<string, string | boolean> = {}
    const active = /active=(true|false)/.exec(node.variant ?? '')?.[1]
    if (active) props.active = active === 'true'
    const slot = slotName('icon', state.icon)
    state.icon += 1
    return {
      kind: 'icon',
      path,
      name: node.name ?? component,
      css: sizing,
      slot,
      icon: {
        component: iconComponent,
        props,
        size: Math.round(Math.max(node.w, node.h)),
        color: tint ? solidCss(tint, env, 'tint') : undefined
      }
    }
  }
  const target = node.componentId ? env.refsById.get(node.componentId) : undefined
  if (target) {
    return {
      kind: 'ref',
      path,
      name: node.name ?? component,
      css: sizing,
      ref: { component: target.name, props: target.props }
    }
  }
  return glyphNode(node, path, parent, env, `unresolved-instance:${component}`)
}

const LINE_TOLERANCE = 0.1

// How many lines a text node holds, read from its geometry: a hugging height is a whole number of its captured
// line height. Whole-line geometry remains meaningful in a fixed-height box too. A height between whole
// lines remains ambiguous.
function capturedLines(node: RawNode, lineHeight: number): number | 'ambiguous' {
  const ratio = node.h / lineHeight
  const lines = Math.round(ratio)
  const whole = lines >= 1 && Math.abs(ratio - lines) <= LINE_TOLERANCE
  if (whole) return lines
  if (node.sizing?.[1] === 'FIXED') return 'ambiguous'
  if (ratio < 1.5) return 1
  return whole ? lines : 'ambiguous'
}

function buildNode(
  node: RawNode,
  path: number[],
  parent: Parent,
  env: ViewEnv,
  state: BuildState
): ViewNode | undefined {
  if (node.hidden) return undefined
  const isRoot = path.length === 0

  if (node.type === 'TEXT') {
    const { textStyle, decls } = resolveTextStyle(node, env)
    const style = textStyle ? env.textStyles.get(textStyle) : undefined
    const lineHeight = style
      ? (style.fontSize * style.lineHeightPercent) / 100
      : (node.fontSize ?? 16) * 1.2
    // The node's own line height is what Figma lays the text out with; the style's only stands in for it.
    const capturedLineHeight =
      node.lineHeight?.unit === 'PIXELS'
        ? node.lineHeight.value
        : node.lineHeight?.unit === 'PERCENT' && node.lineHeight.value !== undefined
          ? ((node.fontSize ?? style?.fontSize ?? 16) * node.lineHeight.value) / 100
          : undefined
    const widthBound = node.sizing?.[0] !== undefined && node.sizing[0] !== 'HUG'
    const styleWraps = widthBound && node.h > lineHeight * 1.5
    const geometryLines = capturedLineHeight ? capturedLines(node, capturedLineHeight) : undefined
    if (widthBound && geometryLines === 'ambiguous') {
      const label = node.name ?? path.join('.')
      env.report.notes.add(`text-wrap-ambiguous:${label}`)
      env.report.manual.add(
        `text-wrap-ambiguous:${label}: the height is not a whole number of captured lines, so it stays on one line`
      )
    }
    const wraps =
      geometryLines === 'ambiguous'
        ? false
        : typeof geometryLines === 'number'
          ? widthBound && geometryLines >= 2
          : styleWraps
    // Figma text nodes sometimes carry a line height that differs from the matched text style's, which
    // changes the height of hugging containers by a pixel or two; the measured node height is the truth.
    if (!node.lineHeight && node.text && !/[\n\u2028\u2029]/.test(node.text)) {
      const lines = wraps ? Math.round(node.h / lineHeight) : 1
      const measured = node.h / lines
      if (Math.abs(measured - lineHeight) > 0.1) decls.push(['line-height', px(measured)])
    }
    const css: CssBlock = {
      textStyle,
      decls: [
        ['display', 'block'],
        ['min-height', px(node.h)],
        ...(node.sizing?.[0] === 'HUG' ? [['width', px(node.w)] as [string, string]] : []),
        ...textPaintDecls(node.fills, env),
        ...(/[\n\u2028\u2029]/.test(node.text ?? '')
          ? ([['white-space', 'pre-wrap']] as [string, string][])
          : wraps
            ? []
            : ([['white-space', 'nowrap']] as [string, string][])),
        ...decls,
        ...sizingDecls({ ...node, sizing: node.sizing ?? ['HUG', 'HUG'] }, parent, false).filter(
          ([property]) => property !== 'height' && (wraps || property !== 'width')
        ),
        ...positionDecls(node, parent, env),
        ...(node.opacity !== undefined
          ? ([['opacity', String(node.opacity)]] as [string, string][])
          : [])
      ]
    }
    if (node.textTruncation === 'ENDING') {
      css.decls.push(['overflow', 'hidden'], ['text-overflow', 'ellipsis'])
      if (node.w > 0) css.decls.push(['max-width', px(node.w)])
      if ((node.maxLines ?? 1) > 1)
        css.decls.push(
          ['display', '-webkit-box'],
          ['-webkit-box-orient', 'vertical'],
          ['-webkit-line-clamp', String(node.maxLines)],
          ['white-space', 'normal']
        )
    }
    const slot = slotName('text', state.text)
    state.text += 1
    const textRuns = node.textRuns?.map((run) => {
      const metrics = resolveTextStyle({ type: 'TEXT', w: 0, h: 0, ...run }, env)
      return {
        text: run.text.replace(/[\u2028\u2029]/g, '\n'),
        css: {
          textStyle: metrics.textStyle,
          decls: [
            ...metrics.decls,
            ...(run.lineHeight ? [] : [['line-height', 'inherit'] as [string, string]]),
            ...textPaintDecls(run.fills, env)
          ]
        }
      }
    })
    return {
      kind: 'text',
      path,
      name: node.name ?? 'text',
      css,
      text: (node.text ?? '').replace(/[\u2028\u2029]/g, '\n'),
      textRuns,
      slot
    }
  }

  // Effective instance children preserve text, visibility and nested overrides from Figma.
  if (node.type === 'INSTANCE' && !node.children)
    return instanceNode(node, path, parent, env, state)

  const artwork = artworkNode(node, path, parent, env)
  if (artwork) return artwork
  if (node.heavy) {
    return glyphNode(
      node,
      path,
      parent,
      env,
      `heavy-artwork:${node.name ?? node.type}: too large to embed in a snapshot`
    )
  }

  if (ARBITRARY_GLYPHS.has(node.type)) {
    if (node.type === 'LINE' && /cursor/i.test(node.name ?? '')) {
      const tint = node.strokes?.[0]
      return {
        kind: 'box',
        path,
        name: 'caret',
        css: {
          decls: [
            [
              'width',
              px(node.strokeWeight === 'mixed' || !node.strokeWeight ? 2 : node.strokeWeight)
            ],
            ['height', px(node.w)],
            ['flex', 'none'],
            ['background', (tint && solidCss(tint, env, 'tint')) || 'currentColor']
          ]
        }
      }
    }
    return glyphNode(node, path, parent, env, `custom-glyph:${node.name ?? node.type}`)
  }

  if (node.type === 'VECTOR' && !BOX_VECTOR_NAME.test(node.name ?? '')) {
    return glyphNode(node, path, parent, env, `custom-glyph:${node.name ?? node.type}`)
  }

  const kids = node.children ?? []
  const hasLayout = Boolean(node.layout)
  const container: Parent = {
    node,
    positioned: !hasLayout && kids.filter((kid) => !kid.hidden).length > 0
  }
  if (kids.length === 0 && !isRoot) {
    return {
      kind: 'box',
      path,
      name: node.name ?? 'box',
      css: leafBoxCss(node, parent, env, node.type === 'ELLIPSE')
    }
  }
  const css = boxCss(node, parent, isRoot, env)
  const masks = kids.filter((kid) => kid.isMask && !kid.hidden)
  if (masks.length) {
    const mask = masks[0]!
    const image = maskBackground(mask, env)
    if (masks.length === 1 && kids[0] === mask && image && mask.maskType !== 'VECTOR') {
      css.decls.push(
        ['mask-image', image],
        ['mask-repeat', 'no-repeat'],
        ['mask-size', `${px(mask.w)} ${px(mask.h)}`],
        ['mask-position', `${px(mask.x ?? 0)} ${px(mask.y ?? 0)}`],
        ['mask-mode', mask.maskType === 'LUMINANCE' ? 'luminance' : 'alpha']
      )
    } else env.report.manual.add('unsupported-mask: capture or grouping requires review')
  }
  const children: ViewNode[] = []
  kids.forEach((kid, index) => {
    if (kid.isMask) return
    const built = buildNode(kid, [...path, index], container, env, state)
    if (built) children.push(built)
  })
  return { kind: 'box', path, name: node.name ?? 'box', css, children }
}

function buildView(root: RawNode, env: ViewEnv): ViewNode {
  const built = buildNode(root, [], undefined, env, { text: 0, icon: 0 })
  if (!built) throw new Error('A component variant cannot be hidden')
  return built
}

// Layers are matched by their name path (not by index) because a state often inserts or removes a layer,
// for example a tooltip, which shifts every later index. Rules are addressed with the rest view's own paths.
function flatten(
  node: ViewNode,
  prefix = '',
  into = new Map<string, ViewNode>()
): Map<string, ViewNode> {
  const walk = (current: ViewNode, trail: string) => {
    into.set(trail, current)
    for (const [key, child] of keyedChildren(trail, current.children ?? [])) walk(child, key)
  }
  walk(node, prefix)
  return into
}

function keyedChildren(parentKey: string, children: readonly ViewNode[]): [string, ViewNode][] {
  const seen = new Map<string, number>()
  return children.map((child) => {
    const base = `${parentKey}/${encodeURIComponent(child.name)}`
    const count = seen.get(base) ?? 0
    seen.set(base, count + 1)
    return [count === 0 ? base : `${base}#${count}`, child]
  })
}

const RESET: Record<string, string> = {
  background: 'transparent',
  'box-shadow': 'none',
  opacity: '1'
}

function stateRules(rest: ViewNode, other: ViewNode, report: Report, label: string): StateRule[] {
  const base = flatten(rest)
  const target = flatten(other)
  const rules: StateRule[] = []
  const added: string[] = []
  for (const [key, node] of base) {
    if (!target.has(key)) rules.push({ path: node.path, decls: [['display', 'none']] })
  }
  for (const [key, node] of target) {
    const original = base.get(key)
    if (!original || original.kind !== node.kind) {
      added.push(node.name)
      continue
    }
    const before = new Map(original.css.decls)
    const after = new Map(node.css.decls)
    const decls: [string, string][] = []
    for (const [property, value] of after) {
      if (before.get(property) !== value) decls.push([property, value])
    }
    for (const property of before.keys()) {
      if (!after.has(property) && property in RESET) decls.push([property, RESET[property]!])
    }
    if (node.css.textStyle !== original.css.textStyle && node.css.textStyle) {
      report.notes.add(`state-text-style-change:${label}:${node.name}`)
    }
    const ring = new Map(original.css.after ?? [])
    const ringDiff = (node.css.after ?? []).filter(
      ([property, value]) => ring.get(property) !== value
    )
    if (decls.length > 0 || ringDiff.length > 0)
      rules.push({
        path: original.path,
        decls,
        ...(ringDiff.length > 0 ? { after: ringDiff } : {})
      })
  }
  if (added.length > 0) {
    report.manual.add(`state-adds-layers:${label}: ${[...new Set(added)].join(', ')}`)
  }
  return rules
}

// Everything a loop does not animate must be identical in both variants: the same layers, kinds, text,
// text runs, slots, icons, references, glyphs and text styles, and the same declared properties. Only
// declaration values may differ (stateRules reports those). Returns the first difference, if any.
function loopPayloadChange(rest: ViewNode, other: ViewNode): string | undefined {
  const base = flatten(rest)
  const target = flatten(other)
  if (base.size !== target.size || [...base.keys()].some((key) => !target.has(key)))
    return 'layers-change'
  const payload = (node: ViewNode): Record<string, unknown> => ({
    kind: node.kind,
    name: node.name,
    text: node.text,
    textRuns: node.textRuns,
    slot: node.slot,
    icon: node.icon,
    ref: node.ref,
    glyph: node.glyph,
    textStyle: node.css.textStyle
  })
  for (const [key, node] of base) {
    const next = target.get(key)!
    const before = payload(node)
    const after = payload(next)
    // Each variant's root is named after its own variant properties (phase=start, phase=end).
    if (key === '') {
      delete before.name
      delete after.name
    }
    for (const field of Object.keys(before))
      if (JSON.stringify(before[field]) !== JSON.stringify(after[field]))
        return `non-geometry-change:${field}`
    const properties = new Set(node.css.decls.map(([property]) => property))
    const nextProperties = new Set(next.css.decls.map(([property]) => property))
    for (const property of new Set([...properties, ...nextProperties]))
      if (properties.has(property) !== nextProperties.has(property))
        return `unpaired-property:${property}`
  }
  return undefined
}

const GRAFT_BASE = 1000

function declOf(node: ViewNode, property: string): string | undefined {
  return node.css.decls.find(([name]) => name === property)?.[1]
}

function setDecl(node: ViewNode, property: string, value: string): void {
  const index = node.css.decls.findIndex(([name]) => name === property)
  if (index >= 0) node.css.decls[index] = [property, value]
  else node.css.decls.push([property, value])
}

// A state that adds a layer (a tooltip on hover) can only be driven by CSS if the layer already exists in the
// rest view. The added subtree is copied into the rest view hidden, and its `display` is made explicit in the
// state view so the ordinary state diff reveals it. Grafted paths start at GRAFT_BASE so they never collide
// with a path the capture produced.
function graftAddedLayers(
  rest: ViewNode,
  other: ViewNode
): { rest: ViewNode; other: ViewNode; grafted: string[] } {
  const restClone = structuredClone(rest)
  const otherClone = structuredClone(other)
  const restIndex = flatten(restClone)
  for (const [key, node] of flatten(otherClone)) {
    const original = restIndex.get(key)
    if (
      node.kind === 'text' &&
      original?.kind === 'text' &&
      (node.text !== original.text ||
        JSON.stringify(node.textRuns) !== JSON.stringify(original.textRuns))
    )
      node.name += ` [text=${JSON.stringify([node.text, node.textRuns])}]`
  }
  const target = flatten(otherClone)
  const grafted: string[] = []
  for (const [key, node] of target) {
    if (
      restIndex.has(key) &&
      declOf(restIndex.get(key)!, 'display') === 'none' &&
      !declOf(node, 'display')
    )
      setDecl(node, 'display', node.kind === 'box' ? 'block' : 'inline')
    if (restIndex.has(key) || grafted.some((prefix) => key.startsWith(`${prefix}/`))) continue
    const cut = key.lastIndexOf('/')
    const parentKey = cut < 0 ? '' : key.slice(0, cut)
    const parent = cut < 0 ? undefined : restIndex.get(parentKey)
    if (!parent) continue
    const used = (parent.children ?? []).map((child) => child.path[child.path.length - 1] ?? 0)
    const slot = Math.max(GRAFT_BASE - 1, ...used) + 1
    const shown = declOf(node, 'display') ?? (node.kind === 'box' ? 'block' : 'inline')
    setDecl(node, 'display', shown)
    const copy = structuredClone(node)
    const renumber = (current: ViewNode, path: number[]) => {
      current.path = path
      current.children?.forEach((child, index) => renumber(child, [...path, index]))
    }
    renumber(copy, [...parent.path, slot])
    setDecl(copy, 'display', 'none')
    const siblings = parent.children ?? []
    const targetParent = target.get(parentKey)
    const targetEntries = keyedChildren(parentKey, targetParent?.children ?? [])
    const targetPosition = targetEntries.findIndex(([targetKey]) => targetKey === key)
    const currentEntries = keyedChildren(parentKey, siblings)
    let insertAt = -1
    for (let index = targetPosition + 1; index < targetEntries.length; index++) {
      const [targetKey, targetSibling] = targetEntries[index]!
      const baseSibling = currentEntries.find(
        ([currentKey, currentSibling]) =>
          currentKey === targetKey && currentSibling.kind === targetSibling.kind
      )?.[1]
      if (baseSibling) {
        insertAt = siblings.indexOf(baseSibling)
        break
      }
    }
    if (insertAt < 0) {
      for (let index = targetPosition - 1; index >= 0; index--) {
        const [targetKey, targetSibling] = targetEntries[index]!
        const baseSibling = currentEntries.find(
          ([currentKey, currentSibling]) =>
            currentKey === targetKey && currentSibling.kind === targetSibling.kind
        )?.[1]
        if (baseSibling) {
          insertAt = siblings.indexOf(baseSibling) + 1
          break
        }
      }
    }
    if (insertAt < 0) insertAt = siblings.length
    parent.children = [...siblings.slice(0, insertAt), copy, ...siblings.slice(insertAt)]
    grafted.push(key)
  }
  persistRings(restClone, otherClone)
  return { rest: restClone, other: otherClone, grafted }
}

// A ring that only exists in one state would be created and destroyed by the state change, which cannot
// transition. Both views get it, hidden by opacity where Figma has no stroke, along with the positioning
// it needs, so a state only changes the ring's opacity or paint.
function persistRings(rest: ViewNode, other: ViewNode): void {
  const restIndex = flatten(rest)
  for (const [key, node] of flatten(other)) {
    const original = restIndex.get(key)
    if (!original || original.kind !== node.kind) continue
    for (const [from, to] of [
      [node, original],
      [original, node]
    ] as const) {
      if (!from.css.after || to.css.after) continue
      to.css.after = from.css.after.map(([property, value]) =>
        property === 'opacity' ? [property, '0'] : [property, value]
      )
      if (!declOf(to, 'position')) setDecl(to, 'position', declOf(from, 'position') ?? 'relative')
    }
  }
}

export { buildView, graftAddedLayers, loopPayloadChange, stateRules }
export type { NumberTokens, RefTarget, Report, ViewEnv }
