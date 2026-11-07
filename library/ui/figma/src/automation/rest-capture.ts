import {
  rawComponentPageSchema,
  type RawComponent,
  type RawInteraction,
  type RawNode,
  type RawPaint
} from '../ir/raw-components.js'
import { checksum } from '../capture/ingest-chunks.js'
import { record, type LiveNode } from './live-file.js'

const number = (value: unknown, fallback = 0): number =>
  typeof value === 'number' && Number.isFinite(value) ? value : fallback
const list = (value: unknown): unknown[] => (Array.isArray(value) ? value : [])
const near = (value: unknown, expected: number): boolean =>
  typeof value === 'number' && Number.isFinite(value) && Math.abs(value - expected) < 1e-9
const hex = (value: unknown) => {
  const color = record(value)
  return (
    '#' +
    ['r', 'g', 'b']
      .map((key) =>
        Math.round(number(color[key]) * 255)
          .toString(16)
          .padStart(2, '0')
      )
      .join('')
  )
}
function gradientTransform(paint: Record<string, unknown>): number[][] {
  const points = list(paint.gradientHandlePositions).map(record)
  if (points.length !== 3) throw new Error('Missing gradient handles')
  const [p, q, r] = points as [
    Record<string, unknown>,
    Record<string, unknown>,
    Record<string, unknown>
  ]
  const x = number(p.x),
    y = number(p.y)
  const a = number(q.x) - x,
    b = number(r.x) - x,
    c = number(q.y) - y,
    d = number(r.y) - y
  const det = a * d - b * c
  if (Math.abs(det) < 1e-10) throw new Error('Degenerate gradient handles')
  const u = [d / det, -b / det, (b * y - d * x) / det]
  const v = [-c / det, a / det, (c * x - a * y) / det]
  return paint.type === 'GRADIENT_LINEAR'
    ? [u, [-0.5 * v[0]!, -0.5 * v[1]!, 0.5 - 0.5 * v[2]!]]
    : [
        [0.5 * u[0]!, 0.5 * u[1]!, 0.5 + 0.5 * u[2]!],
        [0.5 * v[0]!, 0.5 * v[1]!, 0.5 + 0.5 * v[2]!]
      ]
}
function paints(value: unknown, assets: Map<string, string>): RawPaint[] {
  return list(value)
    .map(record)
    .filter((p) => p.visible !== false)
    .map((paint) => {
      const type = String(paint.type)
      if (paint.blendMode && paint.blendMode !== 'NORMAL')
        throw new Error(`Unsupported paint blend mode: ${paint.blendMode}`)
      if (type === 'SOLID')
        return {
          type,
          hex: hex(paint.color),
          opacity: number(paint.opacity, 1) * number(record(paint.color).a, 1)
        }
      if (type === 'GRADIENT_LINEAR' || type === 'GRADIENT_RADIAL')
        return {
          type,
          opacity: number(paint.opacity, 1),
          transform: gradientTransform(paint),
          stops: list(paint.gradientStops)
            .map(record)
            .map((stop) => ({
              position: number(stop.position),
              hex: hex(stop.color),
              alpha: number(record(stop.color).a, 1)
            }))
        }
      if (type === 'IMAGE') {
        const imageRef = String(paint.imageRef ?? '')
        // Bytes are fetched by pull-live; a structured image without them must not be guessed.
        if (!assets.has(`image:${imageRef}`)) throw new Error(`Missing image fill: ${imageRef}`)
        return {
          type,
          imageRef,
          scaleMode: String(paint.scaleMode ?? 'FILL'),
          opacity: number(paint.opacity, 1)
        }
      }
      throw new Error(`Unsupported structured paint: ${type}`)
    })
}
const drawingTypes = new Set([
  'VECTOR',
  'BOOLEAN_OPERATION',
  'ELLIPSE',
  'LINE',
  'STAR',
  'REGULAR_POLYGON'
])
const hasText = (node: LiveNode): boolean =>
  node.type === 'TEXT' || (node.children ?? []).some(hasText)
const hasDrawing = (node: LiveNode): boolean =>
  drawingTypes.has(node.type) || (node.children ?? []).some(hasDrawing)
// A rotated rectangle, such as a tooltip arrow drawn as a diamond, has no axis-aligned structured form, so it
// is exported as a drawing. A mask keeps the structured path, where a rotation still fails closed.
function isRotatedLeafRectangle(node: LiveNode): boolean {
  const transform = list(node.relativeTransform).map(list)
  return (
    node.type === 'RECTANGLE' &&
    !node.isMask &&
    !(node.children ?? []).length &&
    transform.length === 2 &&
    !(
      (near(transform[0]?.[0], 1) || near(transform[0]?.[0], -1)) &&
      near(transform[0]?.[1], 0) &&
      near(transform[1]?.[0], 0) &&
      (near(transform[1]?.[1], 1) || near(transform[1]?.[1], -1))
    )
  )
}
function isArtwork(node: LiveNode): boolean {
  return (
    drawingTypes.has(node.type) ||
    isRotatedLeafRectangle(node) ||
    (!hasText(node) &&
      ((node.children ?? []).some((child) => child.isMask === true) ||
        list(node.fills).some((fill) =>
          ['IMAGE', 'GRADIENT_ANGULAR', 'GRADIENT_DIAMOND'].includes(String(record(fill).type))
        ) ||
        (node.type === 'GROUP' && (node.children ?? []).length > 0) ||
        // Preserve strokes on zero-height/width vectors inside their icon canvas.
        (['INSTANCE', 'FRAME'].includes(node.type) && hasDrawing(node))))
  )
}
// Painted pixels may extend past the node box: an outside or centred stroke, a shadow, a child that
// overflows a group. Figma's export with absolute bounds crops them, so such artwork is exported at its
// render bounds and placed at their offset from the box, which stays the layout footprint.
const RENDER_TOLERANCE = 0.01
function renderBounds(node: LiveNode) {
  if (!Object.hasOwn(node, 'absoluteRenderBounds'))
    throw new Error(`Missing render bounds: ${node.id} "${node.name}"`)
  // Null means Figma renders nothing for the node (handled by paintsNothing, or a mask clipping others).
  if (node.absoluteRenderBounds === null) return undefined
  const rendered = record(node.absoluteRenderBounds)
  const { x, y, width, height } = rendered
  const finite = (value: unknown): value is number =>
    typeof value === 'number' && Number.isFinite(value)
  if (!finite(x) || !finite(y) || !finite(width) || !finite(height))
    throw new Error(`Inconsistent render bounds: ${node.id} "${node.name}"`)
  return { x, y, width, height }
}
function needsRenderedBounds(node: LiveNode): boolean {
  const box = record(node.absoluteBoundingBox)
  // Rotated Figma lines can retain floating-point residue instead of an exact zero.
  if ([box.width, box.height].some((size) => typeof size === 'number' && size < 0.001)) return true
  const rendered = renderBounds(node)
  if (!rendered || !(rendered.width > 0 && rendered.height > 0)) return false
  return (
    rendered.x < number(box.x) - RENDER_TOLERANCE ||
    rendered.y < number(box.y) - RENDER_TOLERANCE ||
    rendered.x + rendered.width > number(box.x) + number(box.width) + RENDER_TOLERANCE ||
    rendered.y + rendered.height > number(box.y) + number(box.height) + RENDER_TOLERANCE
  )
}
const hasVisiblePaint = (paints: unknown) =>
  list(paints)
    .map(record)
    .some((paint) => paint.visible !== false && paint.opacity !== 0)
// Figma reports `absoluteRenderBounds: null` for a drawing that paints nothing, such as a divider whose
// only stroke has opacity 0, and renders no export for it. Such a drawing is captured like a transparent
// node. A null render with visible paint, or on anything but a single drawing, is a contradiction the
// capture refuses. An absent field keeps the normal export.
function paintsNothing(node: LiveNode): boolean {
  if (!Object.hasOwn(node, 'absoluteRenderBounds') || node.absoluteRenderBounds !== null)
    return false
  // A mask clips its siblings even when it paints nothing itself, so it keeps the normal export path.
  if (!(drawingTypes.has(node.type) || isRotatedLeafRectangle(node)) || node.isMask) return false
  if (
    (node.children ?? []).length ||
    hasVisiblePaint(node.fills) ||
    hasVisiblePaint(node.strokes) ||
    hasVisiblePaint(node.effects)
  )
    throw new Error(
      `Figma reports no rendered area for ${node.id} "${node.name}", but it has visible paint`
    )
  return true
}
// Artwork to export at its render bounds instead of its node box: degenerate lines and anything that
// paints outside its box.
function degenerateArtworkIds(node: LiveNode): string[] {
  if (node.visible === false || node.opacity === 0 || paintsNothing(node)) return []
  if (isArtwork(node)) {
    return needsRenderedBounds(node) ? [node.id] : []
  }
  return (node.children ?? []).flatMap(degenerateArtworkIds)
}
function artworkIds(node: LiveNode): string[] {
  if (node.visible === false || node.opacity === 0 || paintsNothing(node)) return []
  return isArtwork(node) ? [node.id] : (node.children ?? []).flatMap(artworkIds)
}
function validateStructuredMask(mask: LiveNode): void {
  const visible = (items: unknown) =>
    list(items)
      .map(record)
      .filter((item) => item.visible !== false)
  const supported = (node: LiveNode, root: boolean): boolean => {
    const transform = list(node.relativeTransform).map(list)
    return (
      (root ? ['FRAME', 'RECTANGLE'].includes(node.type) : node.type === 'RECTANGLE') &&
      (!node.layoutMode || node.layoutMode === 'NONE') &&
      Math.abs(number(node.rotation)) < 1e-6 &&
      (!transform.length ||
        (near(transform[0]?.[0], 1) &&
          near(transform[0]?.[1], 0) &&
          near(transform[1]?.[0], 0) &&
          near(transform[1]?.[1], 1))) &&
      !list(node.rectangleCornerRadii).some((radius) => number(radius) !== 0) &&
      !visible(node.strokes).length &&
      !visible(node.effects).length &&
      visible(node.fills).every((paint) =>
        ['SOLID', 'GRADIENT_LINEAR', 'GRADIENT_RADIAL'].includes(String(paint.type))
      ) &&
      (node.children ?? [])
        .filter((child) => child.visible !== false)
        .every((child) => !child.isMask && supported(child, false))
    )
  }
  if (!['ALPHA', 'LUMINANCE'].includes(String(mask.maskType)) || !supported(mask, true))
    throw new Error(`Unsupported structured mask: ${mask.id}`)
}

function textStyle(style: Record<string, unknown>) {
  const weights: Record<number, string> = {
    100: 'Thin',
    200: 'Extra Light',
    300: 'Light',
    400: 'Regular',
    500: 'Medium',
    600: 'Semi Bold',
    700: 'Bold',
    800: 'Extra Bold',
    900: 'Black'
  }
  const weight = weights[number(style.fontWeight, 400)]
  if (!weight || typeof style.fontFamily !== 'string')
    throw new Error('Unsupported or missing font style')
  return {
    font: `${style.fontFamily} ${weight}${style.italic ? ' Italic' : ''}`,
    fontSize: number(style.fontSize),
    lineHeight: { unit: 'PIXELS', value: number(style.lineHeightPx, number(style.fontSize)) },
    letterSpacing: { unit: 'PIXELS', value: number(style.letterSpacing) },
    textCase: typeof style.textCase === 'string' ? style.textCase : 'ORIGINAL'
  }
}
function captureNode(node: LiveNode, assets: Map<string, string>, parent?: LiveNode): RawNode {
  const box = record(node.absoluteBoundingBox),
    parentBox = record(parent?.absoluteBoundingBox)
  if (typeof box.width !== 'number' || typeof box.height !== 'number')
    throw new Error(`Missing bounds: ${node.id}`)
  const output: RawNode = {
    type: ['INSTANCE', 'SLOT'].includes(node.type) ? 'FRAME' : node.type,
    name: node.name,
    w: box.width,
    h: box.height
  }
  const transform = list(node.relativeTransform).map(list)
  const axisAligned =
    transform.length === 2 &&
    (near(transform[0]?.[0], 1) || near(transform[0]?.[0], -1)) &&
    near(transform[0]?.[1], 0) &&
    near(transform[1]?.[0], 0) &&
    (near(transform[1]?.[1], 1) || near(transform[1]?.[1], -1))
  const flippedX = axisAligned && near(transform[0]?.[0], -1)
  const flippedY = axisAligned && near(transform[1]?.[1], -1)
  if (
    parent &&
    (!['HORIZONTAL', 'VERTICAL'].includes(String(parent.layoutMode)) ||
      node.layoutPositioning === 'ABSOLUTE')
  ) {
    // Absolute boxes lose the parent's coordinate direction after a reflection.
    // Store the local bounding-box origin; CSS reflects around the node's center.
    output.x = axisAligned
      ? number(transform[0]?.[2]) - (flippedX ? box.width : 0)
      : number(box.x) - number(parentBox.x)
    output.y = axisAligned
      ? number(transform[1]?.[2]) - (flippedY ? box.height : 0)
      : number(box.y) - number(parentBox.y)
    if (node.layoutPositioning === 'ABSOLUTE') output.absolute = true
  }
  if ((node.opacity === 0 && !node.isMask) || paintsNothing(node))
    return { ...output, type: 'FRAME', opacity: 0 }
  if (isArtwork(node)) {
    const svg = assets.get(node.id)
    if (!svg) throw new Error(`Missing artwork export: ${node.id}`)
    if (needsRenderedBounds(node)) {
      const rendered = record(node.absoluteRenderBounds)
      if (!(number(rendered.width) > 0 && number(rendered.height) > 0))
        throw new Error(`Missing visible stroke bounds: ${node.id}`)
      if (box.width >= 0.001 && box.height >= 0.001) {
        // Keep the layout footprint; the drawing covers every painted pixel, inside or outside it.
        return {
          ...output,
          type: 'FRAME',
          children: [
            {
              type: 'DRAWING',
              w: number(rendered.width),
              h: number(rendered.height),
              x: number(rendered.x) - number(box.x),
              y: number(rendered.y) - number(box.y),
              absolute: true,
              svg,
              sum: checksum(svg)
            }
          ]
        }
      }
      output.w = number(rendered.width)
      output.h = number(rendered.height)
      if (output.x !== undefined) output.x = number(rendered.x) - number(parentBox.x)
      if (output.y !== undefined) output.y = number(rendered.y) - number(parentBox.y)
    }
    return { ...output, type: 'DRAWING', svg, sum: checksum(svg) }
  }
  if (!['FRAME', 'GROUP', 'COMPONENT', 'INSTANCE', 'SLOT', 'TEXT', 'RECTANGLE'].includes(node.type))
    throw new Error(`Unsupported structured node: ${node.type} (${node.id})`)
  if (transform.length && !axisAligned)
    throw new Error(`Unsupported structured transform: ${node.id}`)
  if (flippedX || flippedY) output.flip = [flippedX, flippedY]
  if (node.isMask) {
    validateStructuredMask(node)
    output.isMask = true
    output.maskType = String(node.maskType)
  }
  const visibleChildren = (node.children ?? []).filter((child) => child.visible !== false)
  const masks = visibleChildren.filter((child) => child.isMask)
  if (masks.length) {
    if (
      masks.length !== 1 ||
      visibleChildren[0] !== masks[0] ||
      ['HORIZONTAL', 'VERTICAL', 'GRID'].includes(String(node.layoutMode)) ||
      list(node.fills).some((fill) => record(fill).visible !== false) ||
      list(node.strokes).some((stroke) => record(stroke).visible !== false) ||
      list(node.effects).some((effect) => record(effect).visible !== false)
    )
      throw new Error(`Unsupported structured mask grouping: ${node.id}`)
    validateStructuredMask(masks[0]!)
  }
  if (!axisAligned && Math.abs(number(node.rotation)) > 1e-6)
    throw new Error(`Unsupported structured rotation: ${node.id}`)
  if (node.layoutMode === 'GRID') throw new Error(`Unsupported grid layout: ${node.id}`)
  if (node.blendMode && !['PASS_THROUGH', 'NORMAL'].includes(String(node.blendMode)))
    throw new Error(`Unsupported blend mode: ${node.id}`)
  output.clipsContent = node.clipsContent === true
  output.opacity = number(node.opacity, 1)
  output.fills = paints(node.fills, assets)
  output.strokes = paints(node.strokes, assets)
  output.strokeWeight = number(node.strokeWeight)
  output.strokeAlign = typeof node.strokeAlign === 'string' ? node.strokeAlign : 'INSIDE'
  if (typeof node.strokesIncludedInLayout === 'boolean')
    output.strokesIncludedInLayout = node.strokesIncludedInLayout
  const individual = record(node.individualStrokeWeights)
  if (Object.keys(individual).length)
    output.strokeWeights = [
      number(individual.top),
      number(individual.right),
      number(individual.bottom),
      number(individual.left)
    ]
  output.dash = list(node.strokeDashes).map((value) => number(value))
  output.radius = Array.isArray(node.rectangleCornerRadii)
    ? node.rectangleCornerRadii.map((value) => number(value))
    : number(node.cornerRadius)
  if (node.layoutMode === 'HORIZONTAL' || node.layoutMode === 'VERTICAL')
    output.layout = {
      mode: node.layoutMode,
      pad: [
        number(node.paddingTop),
        number(node.paddingRight),
        number(node.paddingBottom),
        number(node.paddingLeft)
      ],
      gap: number(node.itemSpacing),
      main: String(node.primaryAxisAlignItems ?? 'MIN'),
      cross: String(node.counterAxisAlignItems ?? 'MIN'),
      wrap: node.layoutWrap === 'WRAP'
    }
  if (parent && ['HORIZONTAL', 'VERTICAL'].includes(String(parent.layoutMode)))
    output.sizing = [
      String(node.layoutSizingHorizontal ?? 'FIXED'),
      String(node.layoutSizingVertical ?? 'FIXED')
    ]
  for (const effect of list(node.effects)
    .map(record)
    .filter((effect) => effect.visible !== false)) {
    if (
      !['DROP_SHADOW', 'INNER_SHADOW', 'LAYER_BLUR', 'BACKGROUND_BLUR'].includes(
        String(effect.type)
      )
    )
      throw new Error(`Unsupported effect: ${effect.type} (${node.id})`)
  }
  output.effects = list(node.effects)
    .map(record)
    .filter((effect) => effect.visible !== false)
    .map((effect) => ({
      type: String(effect.type),
      radius: number(effect.radius),
      color: effect.color ? hex(effect.color) : undefined,
      alpha: number(record(effect.color).a, 1),
      offset: [number(record(effect.offset).x), number(record(effect.offset).y)],
      spread: number(effect.spread)
    }))
  if (node.type === 'TEXT') {
    const style = record(node.style)
    output.text = String(node.characters ?? '')
    Object.assign(output, textStyle(style))
    output.textAlignHorizontal = String(style.textAlignHorizontal ?? 'LEFT')
    output.textAlignVertical = String(style.textAlignVertical ?? 'TOP')
    output.paragraphSpacing = number(style.paragraphSpacing)
    if (typeof style.textTruncation === 'string') output.textTruncation = style.textTruncation
    if (typeof style.maxLines === 'number') output.maxLines = style.maxLines
    const overrides = list(node.characterStyleOverrides),
      table = record(node.styleOverrideTable)
    if (overrides.length) {
      const runs: NonNullable<RawNode['textRuns']> = []
      for (let index = 0; index < output.text.length;) {
        const key = overrides[index] ?? 0
        let end = index + 1
        while (end < output.text.length && (overrides[end] ?? 0) === key) end++
        const override = record(table[String(key)])
        runs.push({
          text: output.text.slice(index, end),
          ...textStyle({ ...style, ...override }),
          fills: paints(override.fills ?? node.fills, assets)
        })
        index = end
      }
      output.textRuns = runs
    }
  }
  const children = (node.children ?? []).filter((child) => child.visible !== false)
  if (children.length) output.children = children.map((child) => captureNode(child, assets, node))
  return output
}
function componentNodes(page: LiveNode): { node: LiveNode; section: string[] }[] {
  const found: { node: LiveNode; section: string[] }[] = []
  const walk = (node: LiveNode, section: string[]) => {
    if (node.visible === false || node.name === 'Sync status') return
    if (node.type === 'COMPONENT_SET' || node.type === 'COMPONENT') found.push({ node, section })
    else
      for (const child of node.children ?? [])
        walk(
          child,
          node.type === 'SECTION' || node.type === 'FRAME' ? [...section, node.name] : section
        )
  }
  for (const child of page.children ?? []) walk(child, [])
  return found
}
function variantProperties(node: LiveNode): Record<string, string> {
  return Object.fromEntries(
    node.name.split(',').map((pair) => {
      const [key, ...value] = pair.split('=')
      return [key!.trim(), value.join('=').trim()]
    })
  )
}
// Prototype "Change to" interactions on a variant root; the mapper decides which ones become CSS motion.
function variantInteractions(node: LiveNode): RawInteraction[] {
  return list(node.interactions)
    .map(record)
    .flatMap((interaction) =>
      list(interaction.actions)
        .map(record)
        .filter(
          (action) => action.navigation === 'CHANGE_TO' && typeof action.destinationId === 'string'
        )
        .map((action) => {
          const transition = record(action.transition)
          const easing = record(transition.easing)
          const curve = record(easing.easingFunctionCubicBezier)
          const trigger = record(interaction.trigger)
          return {
            trigger: String(trigger.type),
            ...(trigger.type === 'AFTER_TIMEOUT' && typeof trigger.timeout === 'number'
              ? { timeout: trigger.timeout }
              : {}),
            destinationId: String(action.destinationId),
            transition:
              typeof transition.type === 'string'
                ? {
                    type: transition.type,
                    duration: number(transition.duration),
                    easing: String(easing.type ?? 'LINEAR'),
                    bezier:
                      easing.type === 'CUSTOM_CUBIC_BEZIER'
                        ? ([curve.x1, curve.y1, curve.x2, curve.y2].map((value) =>
                            number(value)
                          ) as [number, number, number, number])
                        : undefined
                  }
                : undefined
          }
        })
    )
}
function withInteractions<T extends object>(variant: T, node: LiveNode) {
  const interactions = variantInteractions(node)
  return interactions.length ? { ...variant, interactions } : variant
}
function imageRefs(value: unknown): string[] {
  if (Array.isArray(value)) return value.flatMap(imageRefs)
  if (!value || typeof value !== 'object') return []
  const item = value as Record<string, unknown>
  return [
    ...(item.type === 'IMAGE' && typeof item.imageRef === 'string' ? [item.imageRef] : []),
    ...Object.values(item).flatMap(imageRefs)
  ]
}
// Image hashes of every fill a structured (non-artwork) node paints, which capture must embed.
function structuredImageRefs(node: LiveNode): string[] {
  if (node.visible === false || isArtwork(node)) return []
  return [
    ...list(node.fills)
      .map(record)
      .filter((paint) => paint.visible !== false && paint.type === 'IMAGE')
      .map((paint) => String(paint.imageRef)),
    ...(node.children ?? []).flatMap(structuredImageRefs)
  ]
}
function capturePage(page: LiveNode, assets: Map<string, string>) {
  const components: RawComponent[] = componentNodes(page).map(({ node, section }) => {
    const variants =
      node.type === 'COMPONENT_SET'
        ? (node.children ?? []).filter(
            (child) => child.type === 'COMPONENT' && child.visible !== false
          )
        : [node]
    if (!variants.length) throw new Error(`Empty component set: ${node.id}`)
    const hiddenVariants = (node.children ?? [])
      .filter((child) => child.type === 'COMPONENT' && child.visible === false)
      .map(({ id, name }) => ({ id, name }))
    const base = variants[0]!
    const properties = Object.fromEntries(
      Object.entries(record(node.componentPropertyDefinitions)).map(([key, value]) => {
        const definition = record(value)
        return [
          key,
          {
            type: String(definition.type),
            default: definition.type === 'SLOT' ? undefined : definition.defaultValue,
            slotDefault: definition.type === 'SLOT' ? definition.defaultValue : undefined,
            options: definition.variantOptions
          }
        ]
      })
    ) as RawComponent['properties']
    const component: RawComponent = {
      kind: node.type === 'COMPONENT_SET' ? 'set' : 'component',
      id: node.id,
      name: node.name,
      hiddenVariants: hiddenVariants.length ? hiddenVariants : undefined,
      section,
      x: number(record(node.absoluteBoundingBox).x),
      y: number(record(node.absoluteBoundingBox).y),
      properties,
      base: withInteractions(
        {
          id: base.id,
          name: base.name,
          props: node.type === 'COMPONENT_SET' ? variantProperties(base) : {},
          tree: captureNode(base, assets)
        },
        base
      ),
      others: variants.slice(1).map((variant) =>
        withInteractions(
          {
            id: variant.id,
            name: variant.name,
            props: variantProperties(variant),
            patches: [{ at: [], replace: captureNode(variant, assets) }]
          },
          variant
        )
      )
    }
    const refs = [...new Set(imageRefs(component))]
    return refs.length
      ? {
          ...component,
          images: Object.fromEntries(refs.map((ref) => [ref, assets.get(`image:${ref}`)!]))
        }
      : component
  })
  return rawComponentPageSchema.parse({ page: { id: page.id, name: page.name.trim() }, components })
}
export {
  capturePage,
  variantInteractions,
  structuredImageRefs,
  captureNode,
  componentNodes,
  artworkIds,
  degenerateArtworkIds,
  gradientTransform
}
