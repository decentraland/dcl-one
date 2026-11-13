import type { ColorValue, TokenCollection } from '../ir/schemas.js'
import { colorToCss, cssName, TokenIndex } from '../generators/tokens.js'

type ResolvedColor = {
  css: string
  via: 'token' | 'value-match' | 'literal' | 'unresolved'
  token?: string
}

type PaintLike = { hex?: string; opacity?: number; token?: string }

type PaletteEntry = { name: string; hex: string; alpha: number }

const near = (a: number, b: number) => Math.abs(a - b) < 0.011

class ColorResolver {
  private readonly palette: PaletteEntry[] = []
  private readonly byName = new Map<string, PaletteEntry>()

  constructor(collections: TokenCollection[]) {
    const index = new TokenIndex(collections)
    const primitives = collections.find((collection) => collection.name === 'Primitives')
    for (const token of primitives?.variables ?? []) {
      if (token.type !== 'COLOR') continue
      const color = index.resolve(token, primitives!.modes[0]!) as ColorValue
      const entry = { name: token.name, hex: color.hex, alpha: color.alpha }
      this.palette.push(entry)
      this.byName.set(`Primitives/${token.name}`, entry)
    }
  }

  resolve(paint: PaintLike): ResolvedColor {
    const opacity = paint.opacity ?? 1
    if (paint.token) {
      const entry = this.byName.get(paint.token)
      if (entry) {
        const variable = `var(${cssName(entry.name)})`
        if (near(opacity, entry.alpha) || (entry.alpha < 1 && opacity === 1)) {
          return { css: variable, via: 'token', token: paint.token }
        }
        const percent = Math.round((opacity / entry.alpha) * 100)
        return {
          css: `color-mix(in srgb, ${variable} ${percent}%, transparent)`,
          via: 'token',
          token: paint.token
        }
      }
    }
    // A paint with neither a known token nor a colour (a gradient or image passed where a solid colour is
    // expected) has no colour to draw. Defaulting it to black hid such paints as a black ring or fill.
    if (!paint.hex) return { css: 'transparent', via: 'unresolved' }
    const hex = paint.hex.toLowerCase()
    const match = this.palette.find((entry) => entry.hex === hex && near(entry.alpha, opacity))
    if (match) return { css: `var(${cssName(match.name)})`, via: 'value-match', token: match.name }
    return { css: colorToCss({ hex, alpha: opacity }), via: 'literal' }
  }
}

export { ColorResolver }
export type { PaintLike, ResolvedColor }
