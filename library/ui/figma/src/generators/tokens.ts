import { textStyleClass } from './naming.js'
import type {
  ColorValue,
  GradientStyle,
  TextStyle,
  Token,
  TokenCollection,
  TokenValue
} from '../ir/schemas.js'
import { camelCase, kebabCase } from './naming.js'
import { inLayer } from './css-layers.js'
import { header, renderValue, type Renderable } from './render.js'
import type { GeneratedFile, GeneratorContext } from './types.js'

type TokenInput = {
  collections: TokenCollection[]
  textStyles: TextStyle[]
  gradients: GradientStyle[]
}

const CSS_PREFIX = '--dcl-'
const WEIGHTS: Record<string, number> = {
  Regular: 400,
  Medium: 500,
  'Semi Bold': 600,
  Bold: 700,
  'Extra Bold': 800
}

const cssName = (tokenName: string) => `${CSS_PREFIX}${kebabCase(tokenName)}`

function isAlias(value: TokenValue): value is { alias: string; collection: string } {
  return typeof value === 'object' && 'alias' in value
}

function colorToCss({ hex, alpha }: ColorValue): string {
  if (alpha >= 1) return hex
  const [r, g, b] = [1, 3, 5].map((start) => parseInt(hex.slice(start, start + 2), 16))
  return `rgba(${r}, ${g}, ${b}, ${Math.round(alpha * 1000000) / 1000000})`
}

function gradientToCss(gradient: GradientStyle): string {
  const stops = gradient.stops
    .map((stop) => `${stop.hex} ${Math.round(stop.position * 10000) / 100}%`)
    .join(', ')
  return gradient.type === 'linear'
    ? `linear-gradient(${gradient.angleDeg ?? 180}deg, ${stops})`
    : `radial-gradient(ellipse at 50% 50%, ${stops})`
}

class TokenIndex {
  private readonly byCollection = new Map<string, Map<string, Token>>()

  constructor(readonly collections: TokenCollection[]) {
    for (const collection of collections) {
      this.byCollection.set(collection.name, new Map(collection.variables.map((v) => [v.name, v])))
    }
  }

  find(collection: string, name: string): Token {
    const token = this.byCollection.get(collection)?.get(name)
    if (!token) throw new Error(`Unresolved alias ${collection}/${name}`)
    return token
  }

  resolve(token: Token, mode: string): ColorValue | number {
    const value = token.values[mode] ?? Object.values(token.values)[0]
    if (value === undefined) throw new Error(`Token ${token.name} has no values`)
    if (!isAlias(value)) return value
    const target = this.find(value.collection, value.alias)
    const owner = this.collections.find((c) => c.variables.includes(target))
    return this.resolve(
      target,
      owner && owner.modes.includes(mode) ? mode : (owner?.modes[0] ?? mode)
    )
  }
}

function cssValue(value: TokenValue): string {
  if (typeof value === 'number') return `${value}px`
  if (isAlias(value)) return `var(${cssName(value.alias)})`
  return colorToCss(value)
}

function nestedColors(collection: TokenCollection, index: TokenIndex): Renderable {
  const mode = collection.modes[0]!
  const tree: { [key: string]: { [key: string]: Renderable } } = {}
  for (const token of collection.variables) {
    const segments = token.name.split('/')
    const group = camelCase(segments[0]!)
    const key = camelCase(segments[segments.length - 1]!)
    const resolved = index.resolve(token, mode)
    tree[group] ??= {}
    if (key in tree[group]!) throw new Error(`Duplicate token key ${group}.${key}`)
    tree[group]![key] = typeof resolved === 'number' ? resolved : colorToCss(resolved)
  }
  return tree
}

function flatNumbers(collection: TokenCollection, prefix: string, index: TokenIndex): Renderable {
  const mode = collection.modes[0]!
  const out: { [key: string]: Renderable } = {}
  for (const token of collection.variables.filter((v) => v.name.startsWith(prefix))) {
    out[token.name.slice(prefix.length)] = index.resolve(token, mode) as number
  }
  return out
}

function breakpointOrder(layout: TokenCollection, index: TokenIndex) {
  const breakpoints = layout.variables.find((v) => v.name === 'layout/breakpoint-min')
  const modes = layout.modes.map((mode, position) => ({
    mode,
    min: breakpoints ? (index.resolve(breakpoints, mode) as number) : position
  }))
  return modes.sort((a, b) => a.min - b.min)
}

function layoutTokens(layout: TokenCollection, index: TokenIndex) {
  const ordered = breakpointOrder(layout, index)
  const variables = layout.variables.filter((v) => v.name !== 'layout/breakpoint-min')
  const responsive: { [key: string]: Renderable } = {}
  const fontSizes: { [key: string]: Renderable } = {}
  const layoutRefs: { [key: string]: Renderable } = {}
  const fontSizeRefs: { [key: string]: Renderable } = {}
  const breakpoints: { [key: string]: Renderable } = {}

  for (const { mode, min } of ordered) breakpoints[camelCase(mode)] = min

  for (const token of variables) {
    const perMode: { [key: string]: Renderable } = {}
    for (const { mode } of ordered) perMode[camelCase(mode)] = index.resolve(token, mode) as number
    const reference = `var(${cssName(token.name)})`
    if (token.name.startsWith('font-size/')) {
      const key = camelCase(token.name.slice('font-size/'.length))
      fontSizes[key] = perMode
      fontSizeRefs[key] = reference
    } else {
      const key = camelCase(token.name.slice('layout/'.length))
      responsive[key] = perMode
      layoutRefs[key] = reference
    }
  }

  const lines: string[] = []
  const [base, ...rest] = ordered
  if (base) {
    lines.push(':root {')
    for (const token of variables) {
      lines.push(`  ${cssName(token.name)}: ${cssValue(token.values[base.mode]!)};`)
    }
    lines.push('}')
    let previous = base.mode
    for (const { mode, min } of rest) {
      const changed = variables.filter(
        (token) => cssValue(token.values[mode]!) !== cssValue(token.values[previous]!)
      )
      if (changed.length > 0) {
        lines.push('', `@media (min-width: ${min}px) {`, '  :root {')
        for (const token of changed)
          lines.push(`    ${cssName(token.name)}: ${cssValue(token.values[mode]!)};`)
        lines.push('  }', '}')
      }
      previous = mode
    }
  }
  return { responsive, fontSizes, layoutRefs, fontSizeRefs, breakpoints, css: lines.join('\n') }
}

function staticCss(collection: TokenCollection): string {
  const mode = collection.modes[0]!
  const declarations = collection.variables.map(
    (token) => `  ${cssName(token.name)}: ${cssValue(token.values[mode] ?? 0)};`
  )
  return `:root {\n${declarations.join('\n')}\n}`
}

function themeRefs(collection: TokenCollection): Renderable {
  const tree: { [key: string]: { [key: string]: Renderable } } = {}
  for (const token of collection.variables) {
    const segments = token.name.split('/')
    const group = camelCase(segments[0]!)
    tree[group] ??= {}
    tree[group]![camelCase(segments[segments.length - 1]!)] = `var(${cssName(token.name)})`
  }
  return tree
}

function flatRefs(collection: TokenCollection, prefix: string): Renderable {
  const out: { [key: string]: Renderable } = {}
  for (const token of collection.variables.filter((v) => v.name.startsWith(prefix))) {
    out[token.name.slice(prefix.length)] = `var(${cssName(token.name)})`
  }
  return out
}

function typography(styles: TextStyle[], fontSizes: { [key: string]: Renderable }) {
  const entries: { [key: string]: Renderable } = {}
  const css: { [key: string]: Renderable } = {}
  for (const style of styles) {
    const sizeKey = camelCase(style.name)
    const responsive = fontSizes[sizeKey] as { [key: string]: number } | undefined
    const sizeVar = `${CSS_PREFIX}font-size-${kebabCase(style.name)}`
    const fontSize = responsive ? `var(${sizeVar})` : `${style.fontSize}px`
    const lineHeight = style.lineHeightAuto ? 'normal' : Math.round(style.lineHeightPercent) / 100
    const textTransform = style.textCase === 'UPPER' ? 'uppercase' : 'none'
    const letterSpacing = style.letterSpacingPx === 0 ? 'normal' : `${style.letterSpacingPx}px`
    const fontWeight = WEIGHTS[style.fontStyle] ?? 400
    const escapedFont = style.fontFamily.replace(
      /[\\'\n\r\f]/g,
      (character) => `\\${character.charCodeAt(0).toString(16)} `
    )
    const fontFamily = `'${escapedFont}', sans-serif`
    entries[style.name] = {
      fontFamily,
      fontWeight,
      fontSize,
      fontSizePx: style.fontSize,
      lineHeight,
      letterSpacing,
      textTransform
    }
    css[style.name] = [
      `font-family: ${fontFamily};`,
      `font-weight: ${fontWeight};`,
      `font-size: ${fontSize};`,
      `line-height: ${lineHeight};`,
      `letter-spacing: ${letterSpacing};`,
      `text-transform: ${textTransform};`
    ].join(' ')
  }
  return { entries, css }
}

function exportConst(name: string, value: Renderable): string {
  return `export const ${name} = ${renderValue(value)} as const\n`
}

const CSS_HEADER = '/* Generated by figma. Do not edit: changes are overwritten on sync. */'

// Every token output (the combined Review preview and the per-family foundations) is assembled from
// these parts, so both carry exactly the same values.
function tokenParts(input: TokenInput) {
  const index = new TokenIndex(input.collections)
  const collection = (name: string) => {
    const found = input.collections.find((c) => c.name === name)
    if (!found) throw new Error(`Collection ${name} not found`)
    return found
  }
  const primitives = collection('Primitives')
  const semantic = collection('Semantic')
  const spacing = collection('Spacing')
  const layout = collection('Layout')

  const names = new Set<string>()
  for (const token of input.collections.flatMap((c) => c.variables)) {
    const name = cssName(token.name)
    if (names.has(name)) throw new Error(`CSS variable collision: ${name}`)
    names.add(name)
  }

  const layoutData = layoutTokens(layout, index)
  const gradients = Object.fromEntries(
    input.gradients.map((g) => [camelCase(g.name.split('/').pop()!), gradientToCss(g)])
  )
  const { entries: textStyles, css: textStyleCss } = typography(
    input.textStyles,
    layoutData.fontSizes
  )

  const gradientCss = `:root {\n${input.gradients
    .map(
      (g) => `  ${CSS_PREFIX}gradient-${kebabCase(g.name.split('/').pop()!)}: ${gradientToCss(g)};`
    )
    .join('\n')}\n}`

  const gradientRefs = Object.fromEntries(
    input.gradients.map((g) => {
      const slug = g.name.split('/').pop()!
      return [camelCase(slug), `var(${CSS_PREFIX}gradient-${kebabCase(slug)})`]
    })
  )

  return {
    css: {
      primitives: staticCss(primitives),
      semantic: staticCss(semantic),
      spacing: staticCss(spacing),
      gradients: gradientCss,
      layout: layoutData.css,
      textStyles:
        Object.entries(textStyleCss)
          .map(([name, value]) => `:where(.${textStyleClass(name)}) { ${value} }`)
          .join('\n') + '\n'
    },
    exports: {
      palette: exportConst('palette', nestedColors(primitives, index)),
      semantic: exportConst('semantic', nestedColors(semantic, index)),
      space: exportConst('space', flatNumbers(spacing, 'space/', index)),
      radius: exportConst('radius', flatNumbers(spacing, 'radius/', index)),
      breakpoints: exportConst('breakpoints', layoutData.breakpoints),
      layout: exportConst('layout', layoutData.responsive),
      fontSizes: exportConst('fontSizes', layoutData.fontSizes),
      gradients: exportConst('gradients', gradients),
      textStyles: exportConst('textStyles', textStyles),
      textStyleCss: exportConst('textStyleCss', textStyleCss)
    },
    theme: [
      header(),
      exportConst('theme', {
        palette: themeRefs(primitives),
        color: themeRefs(semantic),
        space: flatRefs(spacing, 'space/'),
        radius: flatRefs(spacing, 'radius/'),
        layout: layoutData.layoutRefs,
        fontSize: layoutData.fontSizeRefs,
        gradient: gradientRefs
      }),
      'export type Theme = typeof theme\n'
    ].join('\n')
  }
}

const TEXT_STYLE_NAME = 'export type TextStyleName = keyof typeof textStyles\n'

function generateTokens(input: TokenInput, context: GeneratorContext): GeneratedFile[] {
  void context
  const { css, exports, theme } = tokenParts(input)
  const tokensCss = `${CSS_HEADER}\n\n${inLayer(
    'dcl.foundations',
    [css.primitives, css.semantic, css.spacing, css.gradients, css.layout].join('\n\n')
  )}`
  const tokensTs = [
    header(),
    exports.palette,
    exports.semantic,
    exports.space,
    exports.radius,
    exports.breakpoints,
    exports.layout,
    exports.fontSizes,
    exports.gradients,
    exports.textStyles,
    exports.textStyleCss,
    TEXT_STYLE_NAME
  ].join('\n')

  return [
    { path: 'tokens/tokens.css', content: tokensCss },
    { path: 'tokens/text-styles.module.css', content: inLayer('dcl.foundations', css.textStyles) },
    { path: 'tokens/tokens.ts', content: tokensTs },
    { path: 'tokens/theme.ts', content: theme },
    {
      path: 'tokens/index.ts',
      content: `${header()}export * from './tokens'\nexport * from './theme'\n`
    }
  ]
}

/**
 * The shared foundations, one directory per family (docs/ui-standards.md Foundations). Paths are
 * relative to library/ui/foundations. Responsive font sizes are Layout variables, so their custom
 * properties live in layout.css while the typed sizes and text styles live in typography.
 */
function generateFoundations(input: TokenInput): GeneratedFile[] {
  const { css, exports, theme } = tokenParts(input)
  const cssFile = (...blocks: string[]) =>
    `${CSS_HEADER}\n\n${inLayer('dcl.foundations', blocks.join('\n\n'))}`
  const tsFile = (...blocks: string[]) => [header(), ...blocks].join('\n')
  return [
    { path: 'colors/colors.css', content: cssFile(css.primitives, css.semantic, css.gradients) },
    {
      path: 'colors/colors.ts',
      content: tsFile(exports.palette, exports.semantic, exports.gradients)
    },
    { path: 'spacing/spacing.css', content: cssFile(css.spacing) },
    { path: 'spacing/spacing.ts', content: tsFile(exports.space, exports.radius) },
    { path: 'layout/layout.css', content: cssFile(css.layout) },
    { path: 'layout/layout.ts', content: tsFile(exports.breakpoints, exports.layout) },
    {
      path: 'typography/text-styles.module.css',
      content: inLayer('dcl.foundations', css.textStyles)
    },
    {
      path: 'typography/typography.ts',
      content: tsFile(exports.fontSizes, exports.textStyles, exports.textStyleCss, TEXT_STYLE_NAME)
    },
    { path: 'theme.ts', content: theme }
  ]
}

export { colorToCss, cssName, generateFoundations, generateTokens, gradientToCss, TokenIndex }
export type { TokenInput }
