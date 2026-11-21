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
    let view: ReturnType<typeof buildView>

    describe('and its stops are literal', () => {
      beforeEach(() => {
        view = buildView(card({ strokes: [CERISE], strokeWeight: 4, strokeAlign: 'INSIDE' }), env)
      })

      it('should not draw a solid box-shadow', () => {
        expect(decl(view.css.decls, 'box-shadow')).toBeUndefined()
      })

      it('should position the frame for the overlay', () => {
        expect(decl(view.css.decls, 'position')).toBe('relative')
      })

      it('should draw a gradient ring overlay', () => {
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
    })

    describe('and a stop is bound to a variable', () => {
      let bound: RawPaint

      beforeEach(() => {
        bound = {
          ...CERISE,
          stops: [
            { position: 0, hex: '#ff2d55', alpha: 1, token: 'Primitives/Brand/Ruby' },
            { position: 1, hex: '#c640cd', alpha: 1 }
          ]
        }
        view = buildView(card({ strokes: [bound], strokeWeight: 4, strokeAlign: 'INSIDE' }), env)
      })

      it('should reference the variables of bound stops and keep literal stops literal', () => {
        expect(decl(view.css.after, 'background')).toBe(
          'linear-gradient(180deg, var(Primitives/Brand/Ruby) 0%, #c640cd 100%) border-box'
        )
      })
    })

    describe('and the gradient runs diagonally across a non-square box', () => {
      let diagonal: RawPaint

      beforeEach(() => {
        // A diagonal gradient across a 200 x 100 box: it runs down-right and its line spans the box.
        diagonal = {
          ...CERISE,
          transform: [
            [0.5, 0.5, 0],
            [-0.5, 0.5, 0.5]
          ]
        }
        view = buildView(
          card({ w: 200, h: 100, strokes: [diagonal], strokeWeight: 2, strokeAlign: 'INSIDE' }),
          env
        )
      })

      it('should map the gradient onto the CSS gradient line', () => {
        expect(decl(view.css.after, 'background')).toBe(
          'linear-gradient(153.43deg, #ff2d55 0%, #c640cd 100%) border-box'
        )
      })
    })
  })

  describe('when a stroke paint cannot be drawn', () => {
    let view: ReturnType<typeof buildView>

    describe.each([
      ['a radial gradient', { ...CERISE, type: 'GRADIENT_RADIAL' }],
      ['an image', { type: 'IMAGE', imageRef: 'photo' }]
    ])('and the stroke is %s', (_, paint) => {
      beforeEach(() => {
        view = buildView(card({ strokes: [paint as RawPaint], strokeWeight: 4 }), env)
      })

      it('should not draw a box-shadow', () => {
        expect(decl(view.css.decls, 'box-shadow')).toBeUndefined()
      })

      it('should not draw a ring overlay', () => {
        expect(view.css.after).toBeUndefined()
      })

      it('should report the stroke as manual work instead of drawing it black', () => {
        expect([...env.report.manual].join('\n')).toContain('unsupported-stroke-paint:PlaceCard')
      })
    })

    describe('and a solid paint has no colour', () => {
      beforeEach(() => {
        view = buildView(
          card({ strokes: [{ type: 'SOLID' }], strokeWeight: 1, strokeAlign: 'INSIDE' }),
          env
        )
      })

      it('should not draw a box-shadow', () => {
        expect(decl(view.css.decls, 'box-shadow')).toBeUndefined()
      })

      it('should report the paint instead of defaulting to black', () => {
        expect([...env.report.manual].join('\n')).toContain('unresolved-paint:stroke')
      })
    })
  })

  describe('when deciding whether a stroke takes layout space', () => {
    it.each([
      ['included', true, '12px 12px 12px 12px'],
      ['excluded', false, '8px 8px 8px 8px'],
      ['not captured', undefined, '8px 8px 8px 8px']
    ])('should offset content by the padding of a stroke that is %s', (_, included, padding) => {
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

    describe('and an outside stroke is included in layout', () => {
      let view: ReturnType<typeof buildView>

      beforeEach(() => {
        view = buildView(
          card({
            strokes: [CERISE],
            strokeWeight: 4,
            strokeAlign: 'OUTSIDE',
            strokesIncludedInLayout: true
          }),
          env
        )
      })

      it('should keep the padding unchanged', () => {
        expect(decl(view.css.decls, 'padding')).toBe('8px 8px 8px 8px')
      })

      it('should report the stroke instead of guessing its geometry', () => {
        expect([...env.report.manual].join('\n')).toContain('unsupported-stroke-layout:PlaceCard')
      })
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
    let states: ReturnType<typeof statesOf>

    beforeEach(() => {
      states = statesOf(
        card({ layout: pad(8) }),
        card({
          layout: pad(4),
          strokes: [CERISE],
          strokeWeight: 4,
          strokeAlign: 'INSIDE',
          strokesIncludedInLayout: true
        })
      )
    })

    it('should keep the content offset at rest', () => {
      expect(decl(states.rest.css.decls, 'padding')).toBe('8px 8px 8px 8px')
    })

    it('should position the rest box for the ring overlay', () => {
      expect(decl(states.rest.css.decls, 'position')).toBe('relative')
    })

    it('should hide the ring at rest', () => {
      expect(decl(states.rest.css.after, 'opacity')).toBe('0')
    })

    it('should keep size and content offset and only fade the ring in', () => {
      expect(states.rules).toEqual([{ path: [], decls: [], after: [['opacity', '1']] }])
    })
  })

  describe('when the saved capture grows the hover by its stroke (pad 7 at rest and on hover)', () => {
    let states: ReturnType<typeof statesOf>

    beforeEach(() => {
      states = statesOf(
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
    })

    it('should reproduce Figma faithfully: content at 11 and an outer box 8px larger', () => {
      expect(states.rules).toEqual([
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
    let states: ReturnType<typeof statesOf>

    beforeEach(() => {
      states = statesOf(
        card({ strokes: [CERISE], strokeWeight: 4, strokeAlign: 'INSIDE' }),
        card({})
      )
    })

    it('should keep the ring in the hover state and fade it out', () => {
      expect(states.rules).toEqual([{ path: [], decls: [], after: [['opacity', '0']] }])
    })
  })
})
