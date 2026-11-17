import { beforeEach, describe, expect, it } from 'vitest'
import type { RawNode, RawPaint } from '../ir/raw-components.js'
import type { ColorResolver, PaintLike } from './colors.js'
import { buildView, graftAddedLayers, stateRules, type ViewEnv } from './views.js'

const CERISE: RawPaint = {
  type: 'GRADIENT_LINEAR',
  opacity: 1,
  stops: [
    { position: 0, hex: '#ff2d55', alpha: 1 },
    { position: 1, hex: '#c640cd', alpha: 1 }
  ],
  // Top to bottom, as captured for the PlaceCard hover ring.
  transform: [
    [0, 1, 0],
    [1, 0, 0]
  ]
}

const card = (overrides: Partial<RawNode>): RawNode => ({
  type: 'COMPONENT',
  name: 'PlaceCard',
  w: 374,
  h: 136,
  radius: 23,
  layout: { mode: 'VERTICAL', pad: [8, 8, 8, 8], gap: 0, main: 'MIN', cross: 'MIN', wrap: false },
  children: [{ type: 'FRAME', name: 'Card', w: 358, h: 120 }],
  ...overrides
})

const decl = (decls: [string, string][] | undefined, property: string) =>
  decls?.find(([name]) => name === property)?.[1]

describe('buildView with strokes', () => {
  let env: ViewEnv

  beforeEach(() => {
    const resolve = (paint: PaintLike) =>
      paint.token
        ? { css: `var(${paint.token})`, via: 'token' }
        : paint.hex
          ? { css: paint.hex, via: 'literal' }
          : { css: 'transparent', via: 'unresolved' }
    env = {
      colors: { resolve } as unknown as ColorResolver,
      textStyles: new Map(),
      textStyleAliases: {},
      numbers: { space: new Map(), radius: new Map() },
      icons: new Map(),
      iconAliases: {},
      refsById: new Map(),
      report: { notes: new Set(), manual: new Set() }
    }
  })

  describe('when a frame has an inside gradient stroke', () => {
    it('should draw a gradient ring overlay instead of a solid box-shadow', () => {
      const view = buildView(
        card({ strokes: [CERISE], strokeWeight: 4, strokeAlign: 'INSIDE' }),
        env
      )

      expect(decl(view.css.decls, 'box-shadow')).toBeUndefined()
      expect(decl(view.css.decls, 'position')).toBe('relative')
      expect(view.css.after).toEqual(
        expect.arrayContaining([
          ['content', "''"],
          ['position', 'absolute'],
          ['inset', '0'],
          ['border-width', '4px 4px 4px 4px'],
          ['border-radius', 'inherit'],
          ['background', 'linear-gradient(180deg, #ff2d55 0%, #c640cd 100%) border-box'],
          ['mask-composite', 'exclude'],
          ['opacity', '1']
        ])
      )
    })

    it('should reference the variables of bound stops and keep literal stops literal', () => {
      const bound: RawPaint = {
        ...CERISE,
        stops: [
          { position: 0, hex: '#ff2d55', alpha: 1, token: 'Primitives/Brand/Ruby' },
          { position: 1, hex: '#c640cd', alpha: 1 }
        ]
      }

      const view = buildView(
        card({ strokes: [bound], strokeWeight: 4, strokeAlign: 'INSIDE' }),
        env
      )

      expect(decl(view.css.after, 'background')).toBe(
        'linear-gradient(180deg, var(Primitives/Brand/Ruby) 0%, #c640cd 100%) border-box'
      )
    })

    it('should map a diagonal gradient onto the CSS gradient line of a non-square box', () => {
      // A diagonal gradient across a 200 x 100 box: it runs down-right and its line spans the box.
      const diagonal: RawPaint = {
        ...CERISE,
        transform: [
          [0.5, 0.5, 0],
          [-0.5, 0.5, 0.5]
        ]
      }

      const view = buildView(
        card({ w: 200, h: 100, strokes: [diagonal], strokeWeight: 2, strokeAlign: 'INSIDE' }),
        env
      )

      expect(decl(view.css.after, 'background')).toBe(
        'linear-gradient(153.43deg, #ff2d55 0%, #c640cd 100%) border-box'
      )
    })
  })

  describe('when a stroke paint cannot be drawn', () => {
    it.each([
      ['a radial gradient', { ...CERISE, type: 'GRADIENT_RADIAL' }],
      ['an image', { type: 'IMAGE', imageRef: 'photo' }]
    ])('should report %s stroke as manual work instead of drawing it black', (_, paint) => {
      const view = buildView(card({ strokes: [paint as RawPaint], strokeWeight: 4 }), env)

      expect(decl(view.css.decls, 'box-shadow')).toBeUndefined()
      expect(view.css.after).toBeUndefined()
      expect([...env.report.manual].join('\n')).toContain('unsupported-stroke-paint:PlaceCard')
    })

    it('should report a solid paint without a colour instead of defaulting to black', () => {
      const view = buildView(
        card({ strokes: [{ type: 'SOLID' }], strokeWeight: 1, strokeAlign: 'INSIDE' }),
        env
      )

      expect(decl(view.css.decls, 'box-shadow')).toBeUndefined()
      expect([...env.report.manual].join('\n')).toContain('unresolved-paint:stroke')
    })
  })

  describe('when deciding whether a stroke takes layout space', () => {
    it.each([
      ['included', true, '12px 12px 12px 12px'],
      ['excluded', false, '8px 8px 8px 8px'],
      ['not captured', undefined, '8px 8px 8px 8px']
    ])('should offset content by the padding when the stroke is %s', (_, included, padding) => {
      const view = buildView(
        card({
          strokes: [CERISE],
          strokeWeight: 4,
          strokeAlign: 'INSIDE',
          strokesIncludedInLayout: included
        }),
        env
      )

      expect(decl(view.css.decls, 'padding')).toBe(padding)
    })

    it('should report an outside stroke included in layout instead of guessing its geometry', () => {
      const view = buildView(
        card({
          strokes: [CERISE],
          strokeWeight: 4,
          strokeAlign: 'OUTSIDE',
          strokesIncludedInLayout: true
        }),
        env
      )

      expect(decl(view.css.decls, 'padding')).toBe('8px 8px 8px 8px')
      expect([...env.report.manual].join('\n')).toContain('unsupported-stroke-layout:PlaceCard')
    })
  })
})

describe('PlaceCard horizontal rest and hover states', () => {
  let env: ViewEnv

  beforeEach(() => {
    env = {
      colors: {
        resolve: (paint: PaintLike) => ({ css: paint.hex ?? 'transparent', via: 'literal' })
      } as unknown as ColorResolver,
      textStyles: new Map(),
      textStyleAliases: {},
      numbers: { space: new Map(), radius: new Map() },
      icons: new Map(),
      iconAliases: {},
      refsById: new Map(),
      report: { notes: new Set(), manual: new Set() }
    }
  })

  const statesOf = (rest: RawNode, hover: RawNode) => {
    const grafted = graftAddedLayers(buildView(rest, env), buildView(hover, env))
    return {
      rest: grafted.rest,
      rules: stateRules(grafted.rest, grafted.other, env.report, 'hover')
    }
  }
  const pad = (all: number) => ({
    mode: 'VERTICAL' as const,
    pad: [all, all, all, all] as [number, number, number, number],
    gap: 0,
    main: 'MIN',
    cross: 'MIN',
    wrap: false
  })

  describe('when the live design keeps the outer box (rest pad 8; hover pad 4 plus an included 4px stroke)', () => {
    it('should keep size and content offset and only fade the ring in', () => {
      const { rest, rules } = statesOf(
        card({ layout: pad(8) }),
        card({
          layout: pad(4),
          strokes: [CERISE],
          strokeWeight: 4,
          strokeAlign: 'INSIDE',
          strokesIncludedInLayout: true
        })
      )

      expect(decl(rest.css.decls, 'padding')).toBe('8px 8px 8px 8px')
      expect(decl(rest.css.decls, 'position')).toBe('relative')
      expect(decl(rest.css.after, 'opacity')).toBe('0')
      expect(rules).toEqual([{ path: [], decls: [], after: [['opacity', '1']] }])
    })
  })

  describe('when the saved capture grows the hover by its stroke (pad 7 at rest and on hover)', () => {
    it('should reproduce Figma faithfully: content at 11 and an outer box 8px larger', () => {
      const { rules } = statesOf(
        card({ w: 372, h: 134, layout: pad(7) }),
        card({
          w: 380,
          h: 142,
          layout: pad(7),
          strokes: [CERISE],
          strokeWeight: 4,
          strokeAlign: 'INSIDE',
          strokesIncludedInLayout: true
        })
      )

      expect(rules).toEqual([
        {
          path: [],
          decls: [
            ['padding', '11px 11px 11px 11px'],
            ['width', '380px'],
            ['height', '142px']
          ],
          after: [['opacity', '1']]
        }
      ])
    })
  })

  describe('when the ring is only in the rest state', () => {
    it('should keep the ring in the hover state and fade it out', () => {
      const { rules } = statesOf(
        card({ strokes: [CERISE], strokeWeight: 4, strokeAlign: 'INSIDE' }),
        card({})
      )

      expect(rules).toEqual([{ path: [], decls: [], after: [['opacity', '0']] }])
    })
  })
})
