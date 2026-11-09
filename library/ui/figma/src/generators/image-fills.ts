import { embedsRaster, unsafeSvgReason } from './svg-safety.js'

// A neutral fill a consumer can theme. It keeps the layer's box, position, size and repeat.
const IMAGE_PLACEHOLDER =
  'linear-gradient(var(--dcl-image-placeholder, #8080803d), var(--dcl-image-placeholder, #8080803d))'
const FILL_PROPERTIES = new Set(['background', 'background-image'])
// Functions whose string arguments are image URLs, not text.
const IMAGE_SOURCE_FUNCTIONS = new Set(['image-set', '-webkit-image-set', 'image'])
const SVG_DATA = /^data:image\/svg\+xml(;[^,]*)?,/i
const RASTER_DATA = /^data:image\/(?!svg\+xml[;,])/i
const RASTER_MENTION = /data:image\/(?:png|jpeg|gif|webp)/i
// A nested SVG is a whole quoted attribute value, so its payload ends at the matching quote.
const NESTED_SVG = /(["'])\s*data:image\/svg\+xml(;[^,"']*)?,([\s\S]*?)\1/gi
const SVG_MENTION = /data:image\/svg\+xml/gi
const BASE64 = /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/
const MAX_NESTING = 8

function decodeSvgPayload(params: string | undefined, payload: string): string {
  if (!params?.toLowerCase().includes('base64')) return decodeURIComponent(payload)
  if (!BASE64.test(payload)) throw new Error('invalid base64')
  return Buffer.from(payload, 'base64').toString('utf8')
}

// Whether raster data sits in the SVG or in any SVG nested in it. Each nested payload is decoded and
// classified, so a nested vector-only SVG, percent-encoded or base64, does not count as a raster.
function containsRaster(svg: string, depth = 0): boolean {
  if (RASTER_MENTION.test(svg)) return true
  const nestedSvgs = [...svg.matchAll(NESTED_SVG)]
  if (nestedSvgs.length !== (svg.match(SVG_MENTION)?.length ?? 0))
    throw new Error(
      'Refusing to classify a malformed SVG payload: nested SVG outside an attribute value'
    )
  for (const [, , params, payload] of nestedSvgs) {
    if (depth >= MAX_NESTING)
      throw new Error('Refusing to classify a malformed SVG payload: nested too deep')
    let nested: string
    try {
      nested = decodeSvgPayload(params, payload!)
    } catch (error) {
      throw new Error(`Refusing to classify a malformed SVG payload: ${(error as Error).message}`, {
        cause: error
      })
    }
    if (containsRaster(nested, depth + 1)) return true
  }
  return false
}

function stringEnd(css: string, start: number): number {
  const quote = css[start]
  for (let index = start + 1; index < css.length; index++) {
    if (css[index] === '\\') index++
    else if (css[index] === quote) return index + 1
    else if (css[index] === '\n') break
  }
  throw new Error(`Unterminated string in generated CSS near "${css.slice(start, start + 40)}"`)
}

// Reads url(...) starting at `start`, quoted or not, and returns its payload and where it ends.
function readUrl(css: string, start: number): { end: number; value: string } {
  let index = start + 'url('.length
  while (/\s/.test(css[index] ?? '')) index++
  let value: string
  if (css[index] === '"' || css[index] === "'") {
    const end = stringEnd(css, index)
    value = css.slice(index + 1, end - 1)
    index = end
  } else {
    const end = css.slice(index).search(/[\s)"'(]/)
    if (end < 0)
      throw new Error(`Unterminated url() in generated CSS near "${css.slice(start, start + 40)}"`)
    value = css.slice(index, index + end)
    index += end
  }
  while (/\s/.test(css[index] ?? '')) index++
  if (css[index] !== ')')
    throw new Error(`Malformed url() in generated CSS near "${css.slice(start, start + 40)}"`)
  if (value.includes('\\'))
    throw new Error(`Refusing to classify an escaped url() near "${css.slice(start, start + 40)}"`)
  return { end: index + 1, value }
}

function decodeSvg(url: string, params: string | undefined): string {
  return decodeSvgPayload(params, url.slice(url.indexOf(',') + 1))
}

// Raster content decides, not size: a data image that is not SVG, or a safe SVG drawing a raster <image>.
function isRaster(url: string): boolean {
  if (RASTER_DATA.test(url)) return true
  const svgData = SVG_DATA.exec(url)
  if (!svgData) return false
  const svg = decodeSvg(url, svgData[1])
  if (!containsRaster(svg)) return false
  const unsafe = unsafeSvgReason(svg)
  if (unsafe) throw new Error(`Refusing to replace an unsafe SVG image fill: ${unsafe}`)
  return embedsRaster(svg)
}

function fillProperty(css: string, declaration: number, index: number): string {
  const head = css.slice(declaration, index).replace(/\/\*[\s\S]*?\*\//g, '')
  const property = head.includes(':') ? head.slice(0, head.indexOf(':')).trim().toLowerCase() : ''
  if (!FILL_PROPERTIES.has(property))
    throw new Error(
      `Placeholder image fills only replace backgrounds, refusing a raster in ${property || 'an unknown property'}`
    )
  return property
}

// Replaces every raster image source of a background (a url() or an image-set() string) with the placeholder.
// Comments and text strings are skipped, and a raster anywhere else (masks, borders) fails instead of being
// substituted.
function replaceRasterFills(css: string): { css: string; replaced: number } {
  let output = ''
  let copied = 0
  let replaced = 0
  let declaration = 0
  let functions: string[] = []
  let index = 0
  const replace = (start: number, end: number) => {
    fillProperty(css, declaration, start)
    output += css.slice(copied, start) + IMAGE_PLACEHOLDER
    copied = end
    replaced++
  }
  while (index < css.length) {
    if (css.startsWith('/*', index)) {
      const end = css.indexOf('*/', index + 2)
      if (end < 0) throw new Error('Unterminated comment in generated CSS')
      index = end + 2
    } else if (css[index] === '"' || css[index] === "'") {
      const end = stringEnd(css, index)
      if (IMAGE_SOURCE_FUNCTIONS.has(functions.at(-1) ?? '')) {
        const value = css.slice(index + 1, end - 1)
        if (value.includes('\\'))
          throw new Error(
            `Refusing to classify an escaped image source near "${css.slice(index, index + 40)}"`
          )
        if (isRaster(value.trim())) replace(index, end)
      }
      index = end
    } else if (css[index] === '\\')
      // An escape can spell url( as a function name the scanner would not recognize.
      throw new Error(
        `Refusing to classify an escaped CSS token near "${css.slice(Math.max(0, index - 20), index + 20)}"`
      )
    else if (css[index] === '{' || css[index] === '}' || css[index] === ';') {
      declaration = ++index
      functions = []
    } else if (/^url\(/i.test(css.slice(index, index + 4)) && !/[\w-]/.test(css[index - 1] ?? '')) {
      const { end, value } = readUrl(css, index)
      if (isRaster(value)) replace(index, end)
      index = end
    } else if (css[index] === '(') {
      functions.push((/[\w-]*$/.exec(css.slice(declaration, index))?.[0] ?? '').toLowerCase())
      index++
    } else if (css[index] === ')') {
      functions.pop()
      index++
    } else index++
  }
  return { css: output + css.slice(copied), replaced }
}

export { IMAGE_PLACEHOLDER, replaceRasterFills }
