// Published SVG assets are same-origin files, so they are validated against what Figma exports instead of
// screened for known attacks: anything outside these elements, attributes and values is refused.
const ELEMENTS = new Set([
  'svg',
  'g',
  'defs',
  'path',
  'rect',
  'circle',
  'ellipse',
  'line',
  'polyline',
  'polygon',
  'clipPath',
  'mask',
  'pattern',
  'linearGradient',
  'radialGradient',
  'stop',
  'image',
  'use',
  'filter',
  'feBlend',
  'feColorMatrix',
  'feComposite',
  'feFlood',
  'feGaussianBlur',
  'feMorphology',
  'feOffset'
])

const ATTRIBUTES = new Set([
  'id',
  'x',
  'y',
  'x1',
  'x2',
  'y1',
  'y2',
  'cx',
  'cy',
  'r',
  'rx',
  'ry',
  'fx',
  'fy',
  'd',
  'points',
  'width',
  'height',
  'viewBox',
  'preserveAspectRatio',
  'transform',
  'fill',
  'fill-opacity',
  'fill-rule',
  'clip-path',
  'clip-rule',
  'mask',
  'opacity',
  'stroke',
  'stroke-width',
  'stroke-opacity',
  'stroke-linecap',
  'stroke-linejoin',
  'stroke-miterlimit',
  'stroke-dasharray',
  'stroke-dashoffset',
  'offset',
  'stop-color',
  'stop-opacity',
  'gradientUnits',
  'gradientTransform',
  'spreadMethod',
  'maskUnits',
  'maskContentUnits',
  'clipPathUnits',
  'patternUnits',
  'patternContentUnits',
  'patternTransform',
  'filterUnits',
  'color-interpolation-filters',
  'in',
  'in2',
  'result',
  'mode',
  'operator',
  'type',
  'values',
  'stdDeviation',
  'dx',
  'dy',
  'radius',
  'flood-color',
  'flood-opacity',
  'filter',
  'style',
  'k1',
  'k2',
  'k3',
  'k4',
  'shape-rendering',
  'href',
  'xlink:href',
  'xmlns',
  'xmlns:xlink'
])

const NAMESPACES: Record<string, string> = {
  xmlns: 'http://www.w3.org/2000/svg',
  'xmlns:xlink': 'http://www.w3.org/1999/xlink'
}
const SAFE_HREF = /^(?:#[\w.-]+|data:image\/(?:png|jpeg|gif|webp);base64,[A-Za-z0-9+/=\s]*)$/
const SAFE_URL_REFERENCE = /^url\(#[\w.-]+\)$/
const NUMBER = /^-?(?:\d+\.?\d*|\.\d+)(?:e[-+]?\d+)?$/i
const STYLE_VALUES: Record<string, RegExp> = {
  'mix-blend-mode':
    /^(?:normal|multiply|screen|overlay|darken|lighten|color-dodge|color-burn|hard-light|soft-light|difference|exclusion|hue|saturation|color|luminosity)$/,
  'mask-type': /^(?:alpha|luminance)$/
}
// Figma writes a blend mode and a mask type as inline style: only those declarations, each at most once.
function allowedStyle(value: string): boolean {
  const seen = new Set<string>()
  const declarations = value.split(';').map((part) => part.trim())
  if (declarations.at(-1) === '') declarations.pop()
  if (!declarations.length) return false
  return declarations.every((declaration) => {
    const match = /^([a-z-]+)\s*:\s*([A-Za-z-]+)$/.exec(declaration)
    if (!match || seen.has(match[1]!) || !STYLE_VALUES[match[1]!]?.test(match[2]!)) return false
    seen.add(match[1]!)
    return true
  })
}
// Attributes Figma emits only with these exact shapes: a filter applied by local reference, the blend mode
// and mask type it writes as inline style, feComposite arithmetic coefficients and a rendering hint.
const VALUES: Record<string, (value: string, element: string) => boolean> = {
  filter: (value) => SAFE_URL_REFERENCE.test(value),
  style: allowedStyle,
  k1: (value, element) => element === 'feComposite' && NUMBER.test(value),
  k2: (value, element) => element === 'feComposite' && NUMBER.test(value),
  k3: (value, element) => element === 'feComposite' && NUMBER.test(value),
  k4: (value, element) => element === 'feComposite' && NUMBER.test(value),
  'shape-rendering': (value) => /^(?:auto|optimizeSpeed|crispEdges|geometricPrecision)$/.test(value)
}
const TAG = /^<(\/?)([A-Za-z][\w.:-]*)((?:\s+[\w.:-]+\s*=\s*(?:"[^"<]*"|'[^'<]*'))*)\s*(\/?)>/
const ATTRIBUTE = /([\w.:-]+)\s*=\s*(?:"([^"]*)"|'([^']*)')/g

function unsafeSvgReason(svg: string): string | undefined {
  // Every element carrying each id, so a filter reference resolves only to one <filter> element.
  const ids = new Map<string, string[]>()
  for (const [, element, attributes] of svg.matchAll(/<([A-Za-z][\w.:-]*)((?:\s[^>]*)?)>/g))
    for (const [, attribute, double, single] of attributes!.matchAll(ATTRIBUTE))
      if (attribute === 'id')
        ids.set(double ?? single!, [...(ids.get(double ?? single!) ?? []), element!])
  let rest = svg.trim()
  while (rest) {
    const tag = TAG.exec(rest)
    if (!tag) return `unexpected markup or text near "${rest.slice(0, 40)}"`
    const [whole, closing, name, attributes] = tag
    if (!ELEMENTS.has(name!)) return `element <${name}> is not allowed`
    if (!closing)
      for (const [, attribute, double, single] of attributes!.matchAll(ATTRIBUTE)) {
        const value = (double ?? single)!
        if (!ATTRIBUTES.has(attribute!)) return `attribute ${attribute} is not allowed`
        if (attribute! in NAMESPACES && value !== NAMESPACES[attribute!])
          return `namespace ${attribute}="${value}" is not allowed`
        if ((attribute === 'href' || attribute === 'xlink:href') && !SAFE_HREF.test(value))
          return `${attribute} must reference a local id or an embedded raster`
        // Character references and CSS escapes could spell a reference the checks below would not see.
        if (/[&\\]/.test(value))
          return `${attribute} may not contain character references or escapes`
        if (/url\s*\(/i.test(value) && !SAFE_URL_REFERENCE.test(value))
          return `${attribute} may only reference a local id`
        const allowed = VALUES[attribute!]
        if (allowed && !allowed(value, name!)) return `${attribute}="${value}" is not allowed`
        if (attribute === 'filter') {
          const targets = ids.get(value.slice('url(#'.length, -1)) ?? []
          if (targets.length !== 1 || targets[0] !== 'filter')
            return `filter must reference exactly one <filter> element: ${value}`
        }
      }
    rest = rest.slice(whole!.length).trimStart()
  }
  return undefined
}

const RASTER_HREF = /^data:image\/(?:png|jpeg|gif|webp);base64,/

// Whether an SVG that already passed unsafeSvgReason references an embedded raster. SAFE_HREF admits a raster
// data: href on any element, <use> as well as <image>, so every element's href counts, not only <image>'s.
function embedsRaster(svg: string): boolean {
  let rest = svg.trim()
  while (rest) {
    const tag = TAG.exec(rest)
    if (!tag) return false
    const [whole, closing, , attributes] = tag
    if (!closing)
      for (const [, attribute, double, single] of attributes!.matchAll(ATTRIBUTE))
        if (
          (attribute === 'href' || attribute === 'xlink:href') &&
          RASTER_HREF.test((double ?? single)!)
        )
          return true
    rest = rest.slice(whole!.length).trimStart()
  }
  return false
}

export { embedsRaster, unsafeSvgReason }
