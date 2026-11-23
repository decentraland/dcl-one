import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { RawNode } from '../ir/raw-components.js'
import type { ColorResolver } from './colors.js'
import { buildView, graftAddedLayers, stateRules, type ViewEnv } from './views.js'

describe('buildView', () => {
  describe('when an auto-layout list clips HUG rows', () => {
    let testEnv: ViewEnv
    let row: ReturnType<typeof buildView>
    const buildList = (verticalSizing: 'HUG' | 'FILL' = 'HUG') =>
      buildView(
        {
          type: 'COMPONENT',
          w: 380,
          h: 570,
          children: [
            {
              type: 'FRAME',
              w: 380,
              h: 570,
              clipsContent: true,
              layout: {
                mode: 'VERTICAL',
                pad: [0, 0, 0, 0],
                gap: 8,
                main: 'MIN',
                cross: 'CENTER',
                wrap: false
              },
              children: [
                {
                  type: 'INSTANCE',
                  w: 380,
                  h: 102,
                  sizing: ['FILL', verticalSizing]
                }
              ]
            }
          ]
        } as RawNode,
        testEnv
      )

    beforeEach(() => {
      testEnv = {
        colors: { resolve: () => ({ css: '#000000' }) } as unknown as ColorResolver,
        textStyles: new Map(),
        textStyleAliases: {},
        numbers: { space: new Map(), radius: new Map() },
        icons: new Map(),
        iconAliases: {},
        refsById: new Map(),
        report: { notes: new Set(), manual: new Set() }
      }
    })

    describe('and the rows hug their content', () => {
      beforeEach(() => {
        const list = buildList().children![0]!
        row = list.children![0]!
      })

      it('should prevent captured HUG rows from shrinking in the clipped list', () => {
        expect(row.css.decls).toContainEqual(['flex-shrink', '0'])
      })
    })

    describe('and the rows fill the list', () => {
      beforeEach(() => {
        row = buildList('FILL').children![0]!.children![0]!
      })

      it('should keep FILL rows flexible within the available space', () => {
        expect(row.css.decls).toContainEqual(['flex', '1 1 0'])
      })

      it('should not stop FILL rows from shrinking', () => {
        expect(row.css.decls).not.toContainEqual(['flex-shrink', '0'])
      })
    })
  })

  describe('when a node has multiple fills', () => {
    let env: ViewEnv
    let root: RawNode
    let resolveColor: ReturnType<typeof vi.fn>

    beforeEach(() => {
      resolveColor = vi.fn((paint: { hex?: string; opacity?: number }) => ({
        css: paint.opacity === undefined ? paint.hex : `rgba(${paint.hex}, ${paint.opacity})`
      }))
      env = {
        colors: { resolve: resolveColor } as unknown as ColorResolver,
        textStyles: new Map(),
        textStyleAliases: {},
        numbers: { space: new Map(), radius: new Map() },
        icons: new Map(),
        iconAliases: {},
        refsById: new Map(),
        report: { notes: new Set(), manual: new Set() }
      }
      root = {
        type: 'FRAME',
        name: 'Modal',
        w: 560,
        h: 680,
        fills: [
          { type: 'SOLID', hex: '#4c147c' },
          {
            type: 'GRADIENT_RADIAL',
            transform: [
              [0.59, 0, 0.21],
              [0, 1.67, -1.37]
            ],
            stops: [
              { position: 0, hex: '#ff4bed', alpha: 0.42 },
              { position: 1, hex: '#ff4bed', alpha: 0 }
            ]
          },
          {
            type: 'GRADIENT_RADIAL',
            transform: [
              [0.81, 0, -0.24],
              [0, 1.11, 0.63]
            ],
            stops: [
              { position: 0, hex: '#ff4bed', alpha: 0.3 },
              { position: 1, hex: '#ff4bed', alpha: 0 }
            ]
          }
        ],
        children: []
      } as RawNode
    })

    afterEach(() => {
      vi.resetAllMocks()
    })

    describe('and a frame paints a captured photo', () => {
      beforeEach(() => {
        root.fills = [{ type: 'IMAGE', imageRef: 'photo', scaleMode: 'FILL' }]
      })

      describe('and the photo was captured', () => {
        let svg: string

        beforeEach(() => {
          env.images = { photo: 'data:image/png;base64,AAAA' }
          const background = new Map(buildView(root, env).css.decls).get('background')!
          svg = decodeURIComponent(background.match(/^url\("data:image\/svg\+xml,([^"]*)"\)/)![1]!)
        })

        it('should size the photo to the frame', () => {
          expect(svg).toContain('viewBox="0 0 560 680"')
        })

        it('should draw the photo covering the frame instead of a placeholder colour', () => {
          expect(svg).toContain(
            'preserveAspectRatio="xMidYMid slice" href="data:image/png;base64,AAAA"'
          )
        })

        it('should report no manual work', () => {
          expect(env.report.manual).toEqual(new Set())
        })
      })

      describe('and the photo was not captured', () => {
        it('should keep the placeholder and flag the missing image', () => {
          buildView(root, env)

          expect([...env.report.manual]).toContainEqual(
            'missing-image-asset: the source image was not captured'
          )
        })
      })
    })

    describe('and a frame clips its contents', () => {
      beforeEach(() => {
        root.clipsContent = true
      })

      it('should clip overflowing content instead of painting it over adjacent controls', () => {
        expect(buildView(root, env).css.decls).toContainEqual(['overflow', 'hidden'])
      })
    })

    describe('and a scrolling tab list starts with an alpha mask', () => {
      beforeEach(() => {
        root.children = [
          {
            type: 'FRAME',
            name: 'fade mask',
            w: 240,
            h: 64,
            isMask: true,
            maskType: 'ALPHA',
            mask: {
              svg: '<svg xmlns="http://www.w3.org/2000/svg"><rect width="240" height="64" fill="black"/></svg>'
            }
          },
          { type: 'TEXT', name: 'Tab label', text: 'Campaign', w: 100, h: 20 }
        ]
      })

      it('should use the mask for alpha clipping instead of painting it over the labels', () => {
        expect(buildView(root, env)).toMatchObject({
          css: {
            decls: expect.arrayContaining([
              ['mask-mode', 'alpha'],
              ['mask-size', '240px 64px']
            ])
          },
          children: [{ kind: 'text', text: 'Campaign' }]
        })
      })

      describe('and the mask is a solid area followed by a fading rectangle', () => {
        beforeEach(() => {
          root.children![0]!.mask = undefined
          root.children![0]!.children = [
            {
              type: 'RECTANGLE',
              name: 'visible',
              w: 192,
              h: 64,
              x: 0,
              y: 0,
              fills: [{ type: 'SOLID', hex: '#000000' }]
            },
            {
              type: 'RECTANGLE',
              name: 'fade',
              w: 48,
              h: 64,
              x: 192,
              y: 0,
              fills: [
                {
                  type: 'GRADIENT_LINEAR',
                  transform: [
                    [1, 0, 0],
                    [0, 1, 0]
                  ],
                  stops: [
                    { position: 0, hex: '#000000', alpha: 1 },
                    { position: 1, hex: '#000000', alpha: 0 }
                  ]
                }
              ]
            }
          ]
        })

        it('should reconstruct the mask geometry rather than exporting the invisible mask node', () => {
          expect(
            decodeURIComponent(
              buildView(root, env).css.decls.find(([key]) => key === 'mask-image')![1]
            )
          ).toEqual(expect.stringContaining("<rect width='192'"))
        })
      })

      describe('and the exported mask is missing', () => {
        beforeEach(() => {
          root.children![0]!.mask = undefined
        })

        it('should report the missing mask as manual work', () => {
          buildView(root, env)
          expect(env.report.manual).toContain(
            'unsupported-mask: capture or grouping requires review'
          )
        })
      })
    })

    describe('and text uses a gradient fill', () => {
      beforeEach(() => {
        root = {
          type: 'TEXT',
          name: 'Campaign',
          text: 'Campaign',
          w: 87,
          h: 19,
          fills: [
            {
              type: 'GRADIENT_LINEAR',
              transform: [
                [1, 0, 0],
                [0, 1, 0]
              ],
              stops: [
                { position: 0, hex: '#ff7439', alpha: 1 },
                { position: 1, hex: '#ff2d55', alpha: 1 }
              ]
            }
          ]
        }
      })

      it('should clip the gradient to the text instead of replacing it with black', () => {
        expect(buildView(root, env).css.decls).toEqual(
          expect.arrayContaining([
            ['background-clip', 'text'],
            ['color', 'transparent']
          ])
        )
      })
    })

    describe('and Figma rounds a hugging text box to whole pixels', () => {
      beforeEach(() => {
        root = {
          type: 'TEXT',
          w: 149,
          h: 21,
          text: 'What is Decentraland?',
          font: 'Inter Regular',
          fontSize: 14,
          sizing: ['HUG', 'HUG']
        }
      })
      it('should preserve the captured box rather than accumulating browser fractions', () => {
        expect(buildView(root, env).css.decls).toEqual(
          expect.arrayContaining([
            ['display', 'block'],
            ['width', '149px'],
            ['min-height', '21px']
          ])
        )
      })
    })

    describe('and a Figma text node has a Unicode line separator and explicit alignment', () => {
      beforeEach(() => {
        root = {
          type: 'TEXT',
          w: 326,
          h: 73,
          text: 'Title\u2028Description',
          font: 'Inter Regular',
          fontSize: 16,
          textAlignHorizontal: 'CENTER',
          lineHeight: { unit: 'PERCENT', value: 150 }
        }
      })

      it('should preserve the line break, alignment and source line height', () => {
        expect(buildView(root, env)).toMatchObject({
          text: 'Title\nDescription',
          css: {
            decls: expect.arrayContaining([
              ['text-align', 'center'],
              ['line-height', '1.5'],
              ['white-space', 'pre-wrap']
            ])
          }
        })
      })
    })

    describe('and text has separately styled runs and a two-line limit', () => {
      beforeEach(() => {
        root = {
          type: 'TEXT',
          w: 240,
          h: 48,
          text: 'Shark Backpack',
          font: 'Inter Regular',
          fontSize: 16,
          textTruncation: 'ENDING',
          maxLines: 2,
          textRuns: [
            {
              text: 'Shark ',
              font: 'Inter Bold',
              fontSize: 16,
              fills: [{ type: 'SOLID', hex: '#ff2d55' }]
            },
            {
              text: 'Backpack',
              font: 'Inter Regular',
              fontSize: 16,
              fills: [{ type: 'SOLID', hex: '#000000' }]
            }
          ]
        }
      })

      it('should retain the search highlight and mixed text weights', () => {
        expect(buildView(root, env).textRuns).toMatchObject([
          {
            text: 'Shark ',
            css: {
              decls: expect.arrayContaining([
                ['font-weight', '700'],
                ['color', '#ff2d55']
              ])
            }
          },
          {
            text: 'Backpack',
            css: {
              decls: expect.arrayContaining([
                ['font-weight', '400'],
                ['color', '#000000']
              ])
            }
          }
        ])
      })

      it('should clamp the text to the captured line count', () => {
        expect(buildView(root, env).css.decls).toEqual(
          expect.arrayContaining([
            ['max-width', '240px'],
            ['overflow', 'hidden'],
            ['-webkit-line-clamp', '2']
          ])
        )
      })
    })

    describe('and an instance has effective children', () => {
      beforeEach(() => {
        root = {
          type: 'INSTANCE',
          name: 'NavMenuItem',
          componentId: 'item-default',
          w: 120,
          h: 40,
          children: [{ type: 'TEXT', name: 'Label', text: 'Events', w: 60, h: 20 }]
        }
        env.refsById.set('item-default', { name: 'NavMenuItem', props: { label: 'Wearables' } })
      })

      it('should preserve the instance text instead of rendering the default reference', () => {
        expect(buildView(root, env)).toMatchObject({
          kind: 'box',
          children: [{ kind: 'text', text: 'Events' }]
        })
      })
    })

    describe('and a selected tab has an inside bottom stroke', () => {
      beforeEach(() => {
        root.strokes = [{ type: 'SOLID', hex: '#ff2d55' }]
        root.strokeWeight = 'mixed'
        root.strokeWeights = [0, 0, 4, 0]
        root.strokeAlign = 'INSIDE'
      })

      it('should render only the underline without adding borders to the other sides', () => {
        expect(buildView(root, env).css.decls).toContainEqual([
          'box-shadow',
          'inset 0 -4px 0 0 #ff2d55'
        ])
      })

      describe('and all individual weights are zero', () => {
        beforeEach(() => {
          root.strokeWeights = [0, 0, 0, 0]
        })

        it('should not render a border', () => {
          expect(buildView(root, env).css.decls.some(([key]) => key === 'box-shadow')).toBe(false)
        })
      })

      describe('and the individual weights are missing from an older capture', () => {
        beforeEach(() => {
          root.strokeWeights = undefined
        })

        it('should flag the border as requiring manual review', () => {
          buildView(root, env)
          expect([...env.report.manual]).toContain(
            'missing-stroke-weights: individual border sides were not captured or are unsupported'
          )
        })
      })
    })

    describe('and radial gradient paints have their own opacity', () => {
      beforeEach(() => {
        root.fills = [
          {
            type: 'GRADIENT_RADIAL',
            opacity: 0.5,
            transform: [
              [1, 0, 0],
              [0, 1, 0]
            ],
            stops: [{ position: 0, hex: '#ff4bed', alpha: 0.42 }]
          }
        ]
      })

      it('should multiply SVG stop alpha by the paint opacity', () => {
        expect(
          buildView(root, env)
            .css.decls.find(([key]) => key === 'background')![1]
            .replace(/url\("data:image\/svg\+xml,([^"]+)"\)/g, (_, body: string) =>
              decodeURIComponent(body)
            )
        ).toContain('stop-opacity="0.21"')
      })

      describe('and the captured transform is absent', () => {
        beforeEach(() => {
          ;(root.fills as Exclude<RawNode['fills'], string | undefined>)[0]!.transform = undefined
        })

        it('should preserve paint opacity in the fallback gradient', () => {
          expect(buildView(root, env).css.decls).toContainEqual([
            'background',
            'linear-gradient(180deg, rgba(#ff4bed, 0.21) 0%)'
          ])
        })
      })
    })

    it('should render every paint as a layered background over the base fill', () => {
      const view = buildView(root, env)

      expect(
        view.css.decls
          .find(([key]) => key === 'background')![1]
          .replace(/url\("data:image\/svg\+xml,([^"]+)"\)/g, (_, body: string) =>
            decodeURIComponent(body)
          )
      ).toEqual(
        expect.stringMatching(
          /radialGradient.*stop-opacity="0.3".*radialGradient.*stop-opacity="0.42".*#4c147c/
        )
      )
    })

    describe('and a state replaces a layer whose name contains a slash', () => {
      let result: ReturnType<typeof graftAddedLayers>

      beforeEach(() => {
        root.children = [{ type: 'FRAME', name: 'general/down-arrow', w: 12, h: 12 }]
        const rest = buildView(root, env)
        root.children[0]!.name = 'general/up-arrow'
        result = graftAddedLayers(rest, buildView(root, env))
      })

      it('should graft the replacement layer', () => {
        expect(result.grafted).toEqual(['/general%2Fup-arrow'])
      })

      it('should hide the replaced layer in the state', () => {
        expect(stateRules(result.rest, result.other, env.report, 'hover')).toContainEqual({
          path: [0],
          decls: [['display', 'none']]
        })
      })
    })

    describe('and a state reveals an added layer among captured siblings', () => {
      let restRoot: RawNode
      let stateRoot: RawNode

      beforeEach(() => {
        restRoot = {
          type: 'FRAME',
          w: 200,
          h: 100,
          children: [
            { type: 'FRAME', name: 'Frame 1', w: 180, h: 25 },
            { type: 'FRAME', name: 'Info', w: 100, h: 20 },
            { type: 'FRAME', name: 'Buttons', w: 120, h: 32 }
          ]
        }
        stateRoot = {
          type: 'FRAME',
          w: 200,
          h: 100,
          children: [
            { type: 'TEXT', name: 'Title', text: 'Title', w: 180, h: 25 },
            { type: 'FRAME', name: 'Info', w: 100, h: 20 },
            { type: 'FRAME', name: 'Buttons', w: 120, h: 32 }
          ]
        }
      })

      it('should place the revealed title before its captured Info and Buttons siblings', () => {
        const rest = buildView(restRoot, env)
        const result = graftAddedLayers(rest, buildView(stateRoot, env))

        expect(result.rest.children?.map((child) => child.name)).toEqual([
          'Frame 1',
          'Title',
          'Info',
          'Buttons'
        ])
      })
    })

    describe('and an existing state transition changes only paint', () => {
      let restRoot: RawNode
      let stateRoot: RawNode

      beforeEach(() => {
        restRoot = {
          type: 'FRAME',
          w: 120,
          h: 40,
          children: [
            { type: 'FRAME', name: 'Label', w: 90, h: 20 },
            { type: 'RECTANGLE', name: 'Icon', w: 20, h: 20 }
          ]
        }
        stateRoot = {
          type: 'FRAME',
          w: 120,
          h: 40,
          children: [
            { type: 'FRAME', name: 'Label', w: 90, h: 20 },
            {
              type: 'RECTANGLE',
              name: 'Icon',
              w: 20,
              h: 20,
              fills: [{ type: 'SOLID', hex: '#ff2d55' }]
            }
          ]
        }
      })

      it('should keep the existing sibling order and paths', () => {
        const rest = buildView(restRoot, env)
        const result = graftAddedLayers(rest, buildView(stateRoot, env))

        expect(result.rest.children?.map((child) => [child.name, child.path])).toEqual([
          ['Label', [0]],
          ['Icon', [1]]
        ])
      })
    })

    describe('and the named text style is absent', () => {
      beforeEach(() => {
        root = {
          type: 'TEXT',
          w: 59,
          h: 24,
          text: 'Sign In',
          textStyle: 'button/large',
          font: 'Inter Semi Bold',
          fontSize: 15,
          textCase: 'UPPER',
          letterSpacing: { unit: 'PIXELS', value: 0.46 }
        }
      })

      it('should preserve effective font metrics', () => {
        expect(buildView(root, env).css.decls).toEqual(
          expect.arrayContaining([
            ['font-size', '15px'],
            ['font-weight', '600'],
            ['letter-spacing', '0.46px'],
            ['line-height', '24px'],
            ['text-transform', 'uppercase']
          ])
        )
      })
    })

    describe('and an absolute scrim is drawn before the content', () => {
      let scrim: ReturnType<typeof buildView>
      let content: ReturnType<typeof buildView>

      beforeEach(() => {
        root.layout = { mode: 'VERTICAL', pad: [0, 0, 0, 0], gap: 0, main: 'MIN', cross: 'MIN' }
        root.children = [
          { type: 'RECTANGLE', name: 'Scrim', w: 560, h: 200, x: 0, y: 0, absolute: true },
          { type: 'FRAME', name: 'Content', w: 560, h: 40, children: [] }
        ] as RawNode[]
        ;[scrim, content] = buildView(root, env).children! as [
          ReturnType<typeof buildView>,
          ReturnType<typeof buildView>
        ]
      })

      it('should keep the scrim absolute', () => {
        expect(scrim.css.decls).toContainEqual(['position', 'absolute'])
      })

      it('should keep the content underneath it in flow', () => {
        expect(content.css.decls).toContainEqual(['position', 'relative'])
      })
    })

    describe('and an empty hugging feedback frame is captured', () => {
      beforeEach(() => {
        root.children = [
          {
            type: 'FRAME',
            name: 'Feedback',
            w: 29,
            h: 17,
            sizing: ['HUG', 'HUG'],
            layout: {
              mode: 'HORIZONTAL',
              pad: [0, 0, 0, 0],
              gap: 4,
              main: 'MIN',
              cross: 'CENTER'
            },
            children: []
          }
        ]
      })

      it('should retain its measured height', () => {
        expect(buildView(root, env).children![0]!.css.decls).toContainEqual(['height', '17px'])
      })
    })

    describe('and a state changes the text of a layer', () => {
      let result: ReturnType<typeof graftAddedLayers>

      beforeEach(() => {
        root.children = [{ type: 'TEXT', name: 'Label', w: 90, h: 24, text: 'Wearables' }]
        const rest = buildView(root, env)
        root.children[0]!.text = 'Whats On'
        result = graftAddedLayers(rest, buildView(root, env))
      })

      it('should preserve changed state text in a separately revealed layer', () => {
        expect(result.rest.children).toEqual(
          expect.arrayContaining([
            expect.objectContaining({ kind: 'text', text: 'Whats On', slot: 'label' })
          ])
        )
      })

      it('should hide the rest text in the state', () => {
        expect(stateRules(result.rest, result.other, env.report, 'hover')).toContainEqual({
          path: [0],
          decls: [['display', 'none']]
        })
      })
    })

    describe('and text overrides the font of a named style', () => {
      beforeEach(() => {
        env.textStyles.set('body/md', {
          name: 'body/md',
          fontFamily: 'Inter',
          fontStyle: 'Regular',
          fontSize: 16,
          lineHeightPercent: 150,
          letterSpacingPx: 0,
          textCase: 'ORIGINAL'
        })
        root = {
          type: 'TEXT',
          w: 90,
          h: 24,
          text: 'Discover',
          textStyle: 'body/md',
          font: 'Inter Semi Bold',
          fontSize: 16
        }
      })

      it('should retain the effective font override', () => {
        expect(buildView(root, env).css.decls).toContainEqual(['font-weight', '600'])
      })
    })

    describe('and an image fill names no source image', () => {
      beforeEach(() => {
        root.fills = [{ type: 'IMAGE' }]
      })

      it('should flag a missing image asset instead of treating the placeholder as verified', () => {
        buildView(root, env)
        expect(env.report.manual).toContain(
          'missing-image-asset: the source image was not captured'
        )
      })
    })

    describe('and hover and pressed states reuse an added layer', () => {
      let pressed: ReturnType<typeof graftAddedLayers>

      beforeEach(() => {
        root.children = []
        const rest = buildView(root, env)
        root.children = [{ type: 'FRAME', name: 'general/up-arrow', w: 10, h: 6 }]
        const target = buildView(root, env)
        const hover = graftAddedLayers(rest, target)
        pressed = graftAddedLayers(hover.rest, target)
      })

      it('should reveal the layer in the pressed state', () => {
        expect(stateRules(pressed.rest, pressed.other, env.report, 'pressed')).toContainEqual({
          path: [1000],
          decls: [['display', 'block']]
        })
      })
    })
  })
})

describe('when rendering reflected nodes', () => {
  let env: ViewEnv

  beforeEach(() => {
    env = {
      colors: { resolve: vi.fn(() => ({ css: '#ffffff' })) } as unknown as ColorResolver,
      textStyles: new Map(),
      textStyleAliases: {},
      numbers: { space: new Map(), radius: new Map() },
      icons: new Map(),
      iconAliases: {},
      refsById: new Map(),
      report: { notes: new Set(), manual: new Set() }
    }
  })

  afterEach(() => {
    vi.resetAllMocks()
  })

  describe('and a reflected container holds a reflected text', () => {
    let view: ReturnType<typeof buildView>

    beforeEach(() => {
      view = buildView(
        {
          type: 'FRAME',
          w: 85,
          h: 64,
          flip: [true, false],
          children: [
            { type: 'TEXT', w: 85, h: 19, x: 0, y: 22.5, text: 'Overview', flip: [true, false] }
          ]
        },
        env
      )
    })

    it('should retain the reflection on the container', () => {
      expect(view.css.decls).toContainEqual(['transform', 'scale(-1, 1)'])
    })

    it('should retain the reflection on the text separately', () => {
      expect(view.children?.[0]?.css.decls).toEqual(
        expect.arrayContaining([
          ['left', '0px'],
          ['top', '22.5px'],
          ['transform', 'scale(-1, 1)'],
          ['transform-origin', 'center']
        ])
      )
    })
  })

  describe('and a reflected child falls back to centering', () => {
    let view: ReturnType<typeof buildView>

    beforeEach(() => {
      view = buildView(
        {
          type: 'FRAME',
          w: 100,
          h: 100,
          children: [{ type: 'RECTANGLE', w: 20, h: 20, flip: [false, true] }]
        },
        env
      )
    })

    it('should compose reflection with fallback centering instead of replacing translation', () => {
      expect(view.children?.[0]?.css.decls).toEqual(
        expect.arrayContaining([['transform', 'translate(-50%, -50%) scale(1, -1)']])
      )
    })
  })

  describe('and a node is not reflected', () => {
    it('should leave it without a scale declaration', () => {
      expect(
        buildView({ type: 'FRAME', w: 100, h: 100 }, env).css.decls.some(
          ([key]) => key === 'transform'
        )
      ).toBe(false)
    })
  })
})
