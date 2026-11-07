import { beforeEach, describe, expect, it } from 'vitest'
import {
  degenerateArtworkIds,
  artworkIds,
  captureNode,
  capturePage,
  gradientTransform,
  structuredImageRefs
} from './rest-capture.js'
import { expandVariants } from '../capture/expand-variants.js'
import type { LiveNode } from './live-file.js'

describe('when converting a live REST component page', () => {
  let page: LiveNode
  let component: LiveNode
  let base: LiveNode
  let hover: LiveNode
  let assets: Map<string, string>
  beforeEach(() => {
    assets = new Map()
    base = {
      id: 'base',
      name: 'State=Default',
      type: 'COMPONENT',
      absoluteBoundingBox: { x: 10, y: 20, width: 200, height: 40 },
      children: [
        {
          id: 'label',
          name: 'Label',
          type: 'TEXT',
          absoluteBoundingBox: { x: 20, y: 25, width: 70, height: 20 },
          characters: 'Hello',
          style: { fontFamily: 'Inter', fontSize: 16, fontWeight: 400, lineHeightPx: 20 },
          fills: [{ type: 'SOLID', color: { r: 1, g: 1, b: 1, a: 0.5 }, opacity: 0.5 }]
        }
      ]
    }
    hover = {
      ...structuredClone(base),
      id: 'hover',
      name: 'State=Hover',
      children: [{ ...structuredClone(base.children![0]!), characters: 'Changed' }]
    }
    component = {
      id: 'set',
      name: 'Test',
      type: 'COMPONENT_SET',
      componentPropertyDefinitions: {
        State: { type: 'VARIANT', defaultValue: 'Default', variantOptions: ['Default', 'Hover'] }
      },
      children: [base, hover]
    }
    page = { id: 'page', name: ' Nav bar ', type: 'CANVAS', children: [component] }
  })
  it('should keep the Change to interactions of each variant with their transition', () => {
    base.interactions = [
      {
        trigger: { type: 'ON_HOVER' },
        actions: [
          {
            type: 'NODE',
            destinationId: 'hover',
            navigation: 'CHANGE_TO',
            transition: {
              type: 'SMART_ANIMATE',
              easing: {
                type: 'CUSTOM_CUBIC_BEZIER',
                easingFunctionCubicBezier: { x1: 0.4, y1: 0, x2: 0.2, y2: 1 }
              },
              duration: 0.2
            }
          }
        ]
      },
      {
        trigger: { type: 'ON_CLICK' },
        actions: [{ type: 'NODE', destinationId: null, navigation: 'NAVIGATE' }]
      }
    ]
    const [captured] = capturePage(page, assets).components

    expect(captured!.base.interactions).toEqual([
      {
        trigger: 'ON_HOVER',
        destinationId: 'hover',
        transition: {
          type: 'SMART_ANIMATE',
          duration: 0.2,
          easing: 'CUSTOM_CUBIC_BEZIER',
          bezier: [0.4, 0, 0.2, 1]
        }
      }
    ])
    expect(captured!.others[0]!.interactions).toBeUndefined()
  })
  it('should keep the wait of a timed change and an instant change back', () => {
    base.interactions = [
      {
        trigger: { type: 'AFTER_TIMEOUT', timeout: 0.001 },
        actions: [
          {
            type: 'NODE',
            destinationId: 'hover',
            navigation: 'CHANGE_TO',
            transition: { type: 'SMART_ANIMATE', easing: { type: 'LINEAR' }, duration: 1.5 }
          }
        ]
      }
    ]
    hover.interactions = [
      {
        trigger: { type: 'AFTER_TIMEOUT', timeout: 0.001 },
        actions: [
          { type: 'NODE', destinationId: 'base', navigation: 'CHANGE_TO', transition: null }
        ]
      }
    ]
    const [captured] = capturePage(page, assets).components

    expect(captured!.base.interactions).toEqual([
      {
        trigger: 'AFTER_TIMEOUT',
        timeout: 0.001,
        destinationId: 'hover',
        transition: { type: 'SMART_ANIMATE', duration: 1.5, easing: 'LINEAR', bezier: undefined }
      }
    ])
    expect(captured!.others[0]!.interactions).toEqual([
      { trigger: 'AFTER_TIMEOUT', timeout: 0.001, destinationId: 'base', transition: undefined }
    ])
  })
  it('should retain complete variant trees including text overrides', () => {
    expect(
      expandVariants(capturePage(page, assets).components[0]!)[1]?.tree.children?.[0]?.text
    ).toBe('Changed')
  })
  it('should preserve a structured slot default without coercing it into a variant', () => {
    const defaultValue = { type: 'SLOT', children: [{ nodeId: 'hero', visible: true }] }
    component.componentPropertyDefinitions = {
      State: { type: 'VARIANT', defaultValue: 'Default', variantOptions: ['Default', 'Hover'] },
      'hero#3013:0': { type: 'SLOT', defaultValue }
    }
    const capture = capturePage(page, assets)
    expect(capture.components[0]!.properties['hero#3013:0']).toEqual({
      type: 'SLOT',
      slotDefault: defaultValue
    })
    expect(capture.components[0]!.properties.State).toEqual({
      type: 'VARIANT',
      default: 'Default',
      options: ['Default', 'Hover']
    })
  })
  it('should retain slots with no default and null slot defaults', () => {
    component.componentPropertyDefinitions = {
      missing: { type: 'SLOT' },
      empty: { type: 'SLOT', defaultValue: null }
    }
    const properties = capturePage(page, assets).components[0]!.properties
    expect(properties.missing).toEqual({ type: 'SLOT' })
    expect(properties.empty).toEqual({ type: 'SLOT', slotDefault: null })
  })
  it('should still reject structured defaults for scalar variant properties', () => {
    component.componentPropertyDefinitions = {
      State: { type: 'VARIANT', defaultValue: { value: 'Default' } }
    }
    expect(() => capturePage(page, assets)).toThrow()
  })
  it('should exclude hidden variants consistently with artwork collection and record their identity', () => {
    hover.visible = false
    hover.children = [{ id: 'hidden-art', type: 'VECTOR', name: 'Hidden artwork' }]
    const capture = capturePage(page, assets).components[0]!
    expect(capture.others).toEqual([])
    expect(capture.hiddenVariants).toEqual([{ id: 'hover', name: 'State=Hover' }])
    expect(artworkIds(component)).toEqual([])
  })
  it('should reject a set without visible variants', () => {
    base.visible = false
    hover.visible = false
    expect(() => capturePage(page, assets)).toThrow('Empty component set')
  })
  it('should retain absolute offsets from the parent', () => {
    expect(captureNode(base, assets).children?.[0]).toMatchObject({ x: 10, y: 5 })
  })
  it.each([
    ['included', true, true],
    ['excluded', false, false],
    ['not reported', undefined, undefined]
  ])(
    'should keep whether strokes take layout space when Figma reports them %s',
    (_, live, kept) => {
      base.strokes = [{ type: 'SOLID', color: { r: 1, g: 0, b: 0, a: 1 } }]
      base.strokeWeight = 4
      if (live !== undefined) base.strokesIncludedInLayout = live

      const node = captureNode(base, assets)

      expect(node.strokesIncludedInLayout).toBe(kept)
      expect('strokesIncludedInLayout' in node).toBe(live !== undefined)
    }
  )
  it('should multiply solid color alpha and paint opacity', () => {
    expect(captureNode(base, assets).children?.[0]?.fills).toEqual([
      { type: 'SOLID', hex: '#ffffff', opacity: 0.25 }
    ])
  })
  describe.each([
    [
      'relativeTransform',
      [
        [2, 0, 0],
        [0, 1, 0]
      ],
      'Unsupported structured transform'
    ],
    ['blendMode', 'MULTIPLY', 'Unsupported blend mode'],
    ['layoutMode', 'GRID', 'Unsupported grid layout'],
    ['effects', [{ type: 'NOISE' }], 'Unsupported effect'],
    ['fills', [{ type: 'SOLID', blendMode: 'MULTIPLY' }], 'Unsupported paint blend mode']
  ])('and the capture contains unsupported %s', (key, value, message) => {
    beforeEach(() => {
      base[String(key)] = value
    })
    it('should stop instead of silently dropping the design property', () => {
      expect(() => captureNode(base, assets)).toThrow(String(message))
    })
  })
  describe('and a frame with text paints a photo as its background', () => {
    beforeEach(() => {
      base.fills = [{ type: 'IMAGE', imageRef: 'photo', scaleMode: 'FILL' }]
      hover.fills = [{ type: 'IMAGE', imageRef: 'photo', scaleMode: 'FILL' }]
    })
    it('should list the photo as one the capture must download', () => {
      expect([...new Set(structuredImageRefs(component))]).toEqual(['photo'])
    })
    it('should keep the photo once per component and point the fill at it', () => {
      assets.set('image:photo', 'data:image/png;base64,AAAA')
      const captured = capturePage(page, assets).components[0]!

      expect(captured.images).toEqual({ photo: 'data:image/png;base64,AAAA' })
      expect(captured.base.tree.fills).toEqual([
        { type: 'IMAGE', imageRef: 'photo', scaleMode: 'FILL', opacity: 1 }
      ])
    })
    it('should fail instead of capturing the frame without its photo', () => {
      expect(() => capturePage(page, assets)).toThrow('Missing image fill: photo')
    })
  })
  describe('and a child is hidden', () => {
    beforeEach(() => {
      base.children![0]!.visible = false
    })
    it('should omit the hidden child', () => {
      expect(captureNode(base, assets).children).toBeUndefined()
    })
  })
  describe('and a vector export is missing', () => {
    beforeEach(() => {
      base.children![0]!.type = 'VECTOR'
    })
    it('should refuse an incomplete capture', () => {
      expect(() => captureNode(base, assets)).toThrow('Missing artwork export')
    })
  })
  describe('and a structured mask includes text', () => {
    beforeEach(() => {
      base.children![0]!.isMask = true
    })
    it('should refuse the unsupported mask instead of dropping it', () => {
      expect(() => captureNode(base, assets)).toThrow('Unsupported structured mask')
    })
  })
  describe('and text is rotated', () => {
    beforeEach(() => {
      base.children![0]!.rotation = 0.5
    })
    it('should refuse the unsupported transform', () => {
      expect(() => captureNode(base, assets)).toThrow('Unsupported structured rotation')
    })
  })
  describe('and a component set is empty', () => {
    beforeEach(() => {
      component.children = []
    })
    it('should refuse incomplete component data', () => {
      expect(() => capturePage(page, assets)).toThrow('Empty component set')
    })
  })
  describe('and text has a mixed weight override', () => {
    beforeEach(() => {
      base.children![0]!.characterStyleOverrides = [0, 0, 1, 1, 1]
      base.children![0]!.styleOverrideTable = { '1': { fontWeight: 700 } }
    })
    it('should retain the styled runs', () => {
      expect(
        captureNode(base, assets).children?.[0]?.textRuns?.map((run) => [run.text, run.font])
      ).toEqual([
        ['He', 'Inter Regular'],
        ['llo', 'Inter Bold']
      ])
    })
  })
})

describe('when mapping REST gradient handles', () => {
  let paint: Record<string, unknown>
  beforeEach(() => {
    paint = {
      type: 'GRADIENT_LINEAR',
      gradientHandlePositions: [
        { x: 0, y: 0.5 },
        { x: 1, y: 0.5 },
        { x: 0, y: 0 }
      ]
    }
  })
  it('should preserve the canonical left-to-right gradient', () => {
    expect(gradientTransform(paint).map((row) => row.map((value) => value || 0))).toEqual([
      [1, 0, 0],
      [0, 1, 0]
    ])
  })
  describe('and the handles are degenerate', () => {
    beforeEach(() => {
      paint.gradientHandlePositions = [
        { x: 0, y: 0 },
        { x: 0, y: 0 },
        { x: 0, y: 0 }
      ]
    })
    it('should refuse to invent a transform', () => {
      expect(() => gradientTransform(paint)).toThrow('Degenerate')
    })
  })
  describe('and a handle is missing', () => {
    beforeEach(() => {
      paint.gradientHandlePositions = []
    })
    it('should refuse to invent a transform', () => {
      expect(() => gradientTransform(paint)).toThrow('Missing')
    })
  })
})

describe('when an icon contains a zero-height vector', () => {
  const icon: LiveNode = {
    id: 'icon',
    name: 'Icon start',
    type: 'INSTANCE',
    absoluteBoundingBox: { x: 17, y: 14, width: 20, height: 20 },
    absoluteRenderBounds: { x: 17, y: 14, width: 20, height: 20 },
    children: [
      {
        id: 'line',
        name: 'Vector',
        type: 'VECTOR',
        absoluteBoundingBox: { x: 19, y: 21, width: 14, height: 0 }
      }
    ]
  }
  it('should export the complete icon canvas instead of the degenerate vector', () => {
    expect(artworkIds(icon)).toEqual(['icon'])
    const svg =
      '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><path d="M2 7h14" stroke="black"/></svg>'
    expect(captureNode(icon, new Map([['icon', svg]]))).toMatchObject({
      type: 'DRAWING',
      w: 20,
      h: 20,
      svg
    })
  })
  it('should keep containers with text structured', () => {
    const mixed = {
      ...icon,
      children: [...icon.children!, { id: 'text', name: 'Label', type: 'TEXT' }]
    }
    expect(artworkIds(mixed)).toEqual(['line'])
  })
  it('should not export hidden icons or empty frames', () => {
    expect(artworkIds({ ...icon, visible: false })).toEqual([])
    expect(artworkIds({ ...icon, type: 'FRAME', children: [] })).toEqual([])
  })
  it('should still reject a missing complete icon export', () => {
    expect(() => captureNode(icon, new Map())).toThrow('Missing artwork export: icon')
  })
})

describe('when capturing a standalone horizontal stroke', () => {
  const line: LiveNode = {
    id: 'line',
    name: 'Divider',
    type: 'VECTOR',
    absoluteBoundingBox: { x: 0, y: 56, width: 820, height: 0 },
    absoluteRenderBounds: { x: 0, y: 55.5, width: 820, height: 1 }
  }
  it('should request render bounds and preserve visible stroke dimensions', () => {
    expect(degenerateArtworkIds(line)).toEqual(['line'])
    expect(captureNode(line, new Map([['line', '<svg/>']]))).toMatchObject({ w: 820, h: 1 })
  })
  it('should handle floating-point residue on a rotated cursor line', () => {
    const cursor = {
      ...line,
      type: 'LINE',
      absoluteBoundingBox: { x: 16, y: 31.5, width: 7.868051170589752e-7, height: 18 },
      absoluteRenderBounds: { x: 15.5, y: 31.5, width: 1, height: 18 }
    }
    expect(degenerateArtworkIds(cursor)).toEqual(['line'])
    expect(captureNode(cursor, new Map([['line', '<svg/>']]))).toMatchObject({ w: 1, h: 18 })
  })
  it('should retain layout space when a divider paints outside its bounds', () => {
    const divider = {
      ...line,
      type: 'LINE',
      absoluteBoundingBox: { x: 219, y: -2259, width: 240, height: 10 },
      absoluteRenderBounds: { x: 219, y: -2260, width: 240, height: 1 }
    }
    expect(degenerateArtworkIds(divider)).toEqual(['line'])
    expect(captureNode(divider, new Map([['line', '<svg/>']]))).toMatchObject({
      type: 'FRAME',
      w: 240,
      h: 10,
      children: [{ type: 'DRAWING', w: 240, h: 1, x: 0, y: -1, absolute: true }]
    })
  })
  it('should export at render bounds when painted pixels cross the layout box', () => {
    const crossing = {
      ...line,
      absoluteBoundingBox: { x: 0, y: 0, width: 240, height: 10 },
      absoluteRenderBounds: { x: 0, y: -0.5, width: 240, height: 1 }
    }
    expect(degenerateArtworkIds(crossing)).toEqual(['line'])
    expect(captureNode(crossing, new Map([['line', '<svg/>']]))).toMatchObject({
      type: 'FRAME',
      w: 240,
      h: 10,
      children: [{ type: 'DRAWING', w: 240, h: 1, x: 0, y: -0.5, absolute: true }]
    })
  })
  it('should reject a stroke with no visible bounds', () => {
    expect(() =>
      captureNode(
        { ...line, absoluteRenderBounds: { x: 0, y: 56, width: 820, height: 0 } },
        new Map([['line', '<svg/>']])
      )
    ).toThrow('Missing visible stroke bounds')
  })
})

describe('when Figma reports that a drawing paints nothing', () => {
  const stroke = { type: 'SOLID', color: { r: 0, g: 0, b: 0, a: 1 } }
  const divider: LiveNode = {
    id: 'div',
    name: 'Div',
    type: 'LINE',
    absoluteBoundingBox: { x: 1615, y: 374, width: 53, height: 0 },
    absoluteRenderBounds: null,
    strokes: [{ ...stroke, opacity: 0 }],
    fills: []
  }

  it('should not request an export and keep the footprint of a divider with a transparent stroke', () => {
    expect(artworkIds(divider)).toEqual([])
    expect(degenerateArtworkIds(divider)).toEqual([])
    expect(captureNode(divider, new Map())).toEqual({
      type: 'FRAME',
      name: 'Div',
      w: 53,
      h: 0,
      opacity: 0
    })
  })

  it.each([
    ['a hidden stroke', { strokes: [{ ...stroke, visible: false }] }],
    ['no paint at all', { strokes: [] }]
  ])('should treat %s as painting nothing', (_, paint) => {
    expect(artworkIds({ ...divider, ...paint })).toEqual([])
  })

  it.each([
    ['a stroke without an opacity field', { strokes: [stroke] }],
    ['a visible fill', { fills: [{ ...stroke, opacity: 0.5 }] }],
    ['a visible shadow', { effects: [{ type: 'DROP_SHADOW', visible: true }] }],
    ['a child drawing', { type: 'BOOLEAN_OPERATION', children: [{ ...divider, id: 'part' }] }]
  ])('should refuse to skip a drawing with %s', (_, change) => {
    const node = { ...divider, ...change }
    const message = 'Figma reports no rendered area for div "Div", but it has visible paint'
    expect(() => artworkIds(node)).toThrow(message)
    expect(() => captureNode(node, new Map([['div', '<svg/>']]))).toThrow(message)
  })

  it('should keep exporting a visible zero-height line', () => {
    const visible = {
      ...divider,
      strokes: [stroke],
      absoluteRenderBounds: { x: 1615, y: 373.5, width: 53, height: 1 }
    }
    expect(degenerateArtworkIds(visible)).toEqual(['div'])
    expect(captureNode(visible, new Map([['div', '<svg/>']]))).toMatchObject({ w: 53, h: 1 })
  })

  it('should keep the normal export when the render bounds field is absent', () => {
    const absent = Object.fromEntries(
      Object.entries(divider).filter(([key]) => key !== 'absoluteRenderBounds')
    ) as LiveNode
    expect(artworkIds(absent)).toEqual(['div'])
    expect(() => captureNode(absent, new Map())).toThrow('Missing artwork export: div')
  })

  it('should keep an unpainted mask on the normal, fail-closed path', () => {
    const mask = { ...divider, isMask: true, maskType: 'ALPHA' }
    expect(artworkIds(mask)).toEqual(['div'])
    expect(degenerateArtworkIds(mask)).toEqual(['div'])
    expect(() => captureNode(mask, new Map([['div', '<svg/>']]))).toThrow(
      'Missing visible stroke bounds: div'
    )
  })

  it('should leave containers to the normal export', () => {
    const frame = { ...divider, type: 'FRAME', children: [{ ...divider, id: 'line' }] }
    expect(() => artworkIds(frame)).not.toThrow()
  })
})

describe('when artwork is inside an entirely transparent container', () => {
  const invisible: LiveNode = {
    id: 'carousel',
    name: 'CAROUSEL',
    type: 'INSTANCE',
    opacity: 0,
    absoluteBoundingBox: { x: 0, y: 0, width: 820, height: 200 },
    children: [{ id: 'icon', name: 'Masked Icon', type: 'VECTOR', absoluteRenderBounds: null }]
  }
  it('should not request exports which Figma cannot render', () => {
    expect(artworkIds(invisible)).toEqual([])
    expect(degenerateArtworkIds(invisible)).toEqual([])
  })
  it('should preserve the transparent layout footprint without requiring artwork', () => {
    expect(captureNode(invisible, new Map())).toEqual({
      type: 'FRAME',
      name: 'CAROUSEL',
      w: 820,
      h: 200,
      opacity: 0
    })
  })
  it('should normalize a slot container to a frame', () => {
    const slot = { ...invisible, type: 'SLOT', opacity: 1, children: [] }
    expect(captureNode(slot, new Map())).toMatchObject({ type: 'FRAME', w: 820, h: 200 })
  })
})

describe('when capturing nested axis reflections', () => {
  const text: LiveNode = {
    id: 'text',
    type: 'TEXT',
    name: 'Overview',
    characters: 'Overview',
    rotation: -180,
    relativeTransform: [
      [-1, 0, 85],
      [0, 1, 22.5]
    ],
    absoluteBoundingBox: { x: 100, y: 222.5, width: 85, height: 19 },
    style: { fontFamily: 'Inter', fontWeight: 400, fontSize: 16 }
  }
  const tab: LiveNode = {
    id: 'tab',
    type: 'INSTANCE',
    name: 'Tabs',
    rotation: -180,
    relativeTransform: [
      [-1, 0, 85],
      [0, 1, 0]
    ],
    absoluteBoundingBox: { x: 100, y: 200, width: 85, height: 64 },
    children: [text]
  }
  it('should preserve both flips and local text placement instead of using world coordinates', () => {
    const parent: LiveNode = {
      id: 'parent',
      name: 'Parent',
      type: 'FRAME',
      absoluteBoundingBox: { x: 90, y: 180, width: 100, height: 100 }
    }
    const raw = captureNode(tab, new Map(), parent)
    expect(raw).toMatchObject({ x: 0, y: 0, flip: [true, false] })
    expect(raw.children?.[0]).toMatchObject({
      x: 0,
      y: 22.5,
      flip: [true, false],
      text: 'Overview'
    })
  })
  it('should preserve a vertical reflection around its local box', () => {
    const raw = captureNode(
      {
        ...text,
        relativeTransform: [
          [1, 0, 10],
          [0, -1, 50]
        ]
      },
      new Map(),
      tab
    )
    expect(raw).toMatchObject({ x: 10, y: 31, flip: [false, true] })
  })
  it('should accept numerical identity residue without inventing a reflection', () => {
    const raw = captureNode(
      {
        ...text,
        rotation: -9.213866198408551e-15,
        relativeTransform: [
          [1, -1.6081230200044232e-16, 16],
          [1.6081230200044232e-16, 1, 13.5]
        ]
      },
      new Map(),
      tab
    )
    expect(raw).toMatchObject({ x: 16, y: 13.5 })
    expect(raw.flip).toBeUndefined()
  })
  it('should reject a nontrivial off-axis coefficient', () => {
    expect(() =>
      captureNode(
        {
          ...text,
          relativeTransform: [
            [1, 0.000001, 0],
            [0, 1, 0]
          ]
        },
        new Map()
      )
    ).toThrow('Unsupported structured transform')
  })
  it('should still reject shear instead of guessing its geometry', () => {
    expect(() =>
      captureNode(
        {
          ...text,
          relativeTransform: [
            [1, 0.5, 0],
            [0, 1, 0]
          ]
        },
        new Map()
      )
    ).toThrow('Unsupported structured transform')
  })
})

describe('when a scrolling label group starts with a structured fade mask', () => {
  let parent: LiveNode
  let mask: LiveNode
  beforeEach(() => {
    mask = {
      id: 'mask',
      name: 'fade (mask)',
      type: 'FRAME',
      isMask: true,
      maskType: 'ALPHA',
      layoutMode: 'NONE',
      absoluteBoundingBox: { x: 64, y: 0, width: 683, height: 64 },
      fills: [],
      children: [
        {
          id: 'solid',
          name: 'visible',
          type: 'RECTANGLE',
          absoluteBoundingBox: { x: 64, y: 0, width: 635, height: 64 },
          fills: [{ type: 'SOLID', color: { r: 0, g: 0, b: 0, a: 1 } }]
        },
        {
          id: 'fade',
          name: 'gradient',
          type: 'RECTANGLE',
          absoluteBoundingBox: { x: 699, y: 0, width: 48, height: 64 },
          fills: [
            {
              type: 'GRADIENT_LINEAR',
              gradientHandlePositions: [
                { x: 0, y: 0.5 },
                { x: 1, y: 0.5 },
                { x: 0, y: 1 }
              ],
              gradientStops: [
                { position: 0, color: { r: 0, g: 0, b: 0, a: 1 } },
                { position: 1, color: { r: 0, g: 0, b: 0, a: 0 } }
              ]
            }
          ]
        }
      ]
    }
    parent = {
      id: 'group',
      name: 'Tab list',
      type: 'FRAME',
      layoutMode: 'NONE',
      absoluteBoundingBox: { x: 64, y: 0, width: 683, height: 64 },
      children: [
        mask,
        {
          id: 'text',
          name: 'Overview',
          type: 'TEXT',
          characters: 'Overview',
          absoluteBoundingBox: { x: 64, y: 22.5, width: 85, height: 19 },
          style: { fontFamily: 'Inter', fontWeight: 400, fontSize: 16 }
        }
      ]
    }
  })
  it('should preserve the alpha mask geometry and keep its sibling text structured', () => {
    const raw = captureNode(parent, new Map())
    expect(artworkIds(parent)).toEqual([])
    expect(raw.children?.[0]).toMatchObject({
      isMask: true,
      maskType: 'ALPHA',
      w: 683,
      h: 64,
      children: [
        { type: 'RECTANGLE', w: 635, x: 0 },
        {
          type: 'RECTANGLE',
          w: 48,
          x: 635,
          fills: [{ type: 'GRADIENT_LINEAR', stops: [{ alpha: 1 }, { alpha: 0 }] }]
        }
      ]
    })
    expect(raw.children?.[1]).toMatchObject({ type: 'TEXT', text: 'Overview' })
  })
  it('should preserve a fully transparent mask rather than expose its siblings', () => {
    mask.opacity = 0
    expect(captureNode(parent, new Map()).children?.[0]).toMatchObject({ isMask: true, opacity: 0 })
  })
  it('should reject a mask after an unmasked sibling', () => {
    parent.children!.reverse()
    expect(() => captureNode(parent, new Map())).toThrow('Unsupported structured mask grouping')
  })
  it('should reject multiple masks and masked parent paint', () => {
    parent.children!.push({ ...mask, id: 'second' })
    expect(() => captureNode(parent, new Map())).toThrow('Unsupported structured mask grouping')
    parent.children!.pop()
    parent.fills = [{ type: 'SOLID', color: { r: 1, g: 0, b: 0 } }]
    expect(() => captureNode(parent, new Map())).toThrow('Unsupported structured mask grouping')
  })
  it('should reject vector masks and mask effects instead of discarding them', () => {
    mask.maskType = 'VECTOR'
    expect(() => captureNode(parent, new Map())).toThrow('Unsupported structured mask')
    mask.maskType = 'ALPHA'
    mask.effects = [{ type: 'LAYER_BLUR', radius: 2 }]
    expect(() => captureNode(parent, new Map())).toThrow('Unsupported structured mask')
  })
})

describe('when a component contains a rotated rectangle', () => {
  const diagonal = Math.SQRT1_2
  const peak: LiveNode = {
    id: 'peak',
    name: 'Peak',
    type: 'RECTANGLE',
    relativeTransform: [
      [diagonal, -diagonal, 0],
      [diagonal, diagonal, 0]
    ],
    absoluteBoundingBox: { x: 204, y: 680, width: 36.77, height: 36.77 },
    absoluteRenderBounds: { x: 206.6, y: 682.49, width: 31.8, height: 31.8 },
    fills: [{ type: 'SOLID', color: { r: 0.086, g: 0.082, b: 0.094, a: 1 } }]
  }
  const svg =
    '<svg xmlns="http://www.w3.org/2000/svg" width="37" height="37"><path d="M0 0"/></svg>'

  it('should export a rotated leaf rectangle as a drawing', () => {
    expect(artworkIds(peak)).toEqual(['peak'])
    expect(captureNode(peak, new Map([['peak', svg]]))).toMatchObject({
      type: 'DRAWING',
      name: 'Peak',
      svg
    })
  })

  it('should keep an axis-aligned rectangle structured', () => {
    const square = {
      ...peak,
      relativeTransform: [
        [1, 0, 0],
        [0, 1, 0]
      ]
    }
    expect(artworkIds(square)).toEqual([])
    expect(captureNode(square, new Map())).toMatchObject({ type: 'RECTANGLE' })
  })

  it.each([
    [
      'a rotated frame with children',
      {
        type: 'FRAME',
        children: [
          {
            ...peak,
            id: 'inner',
            relativeTransform: [
              [1, 0, 0],
              [0, 1, 0]
            ]
          }
        ]
      }
    ],
    ['a rotated mask', { isMask: true }]
  ])('should still refuse %s', (_, change) => {
    const node = { ...peak, ...change } as LiveNode
    expect(artworkIds(node)).not.toContain('peak')
    expect(() => captureNode(node, new Map([['peak', svg]]))).toThrow(
      'Unsupported structured transform: peak'
    )
  })

  // REST capture records every fill as a literal; variable bindings are not kept on any node.
  it.each([
    [
      'a variable-bound fill',
      { boundVariables: { color: { type: 'VARIABLE_ALIAS', id: 'VariableID:1:2' } } }
    ],
    ['an unbound fill', {}]
  ])('should record %s as the same literal colour a structured rectangle gets', (_, binding) => {
    const fill = { type: 'SOLID', color: { r: 0.086, g: 0.082, b: 0.094, a: 1 }, ...binding }
    const square = {
      ...peak,
      relativeTransform: [
        [1, 0, 0],
        [0, 1, 0]
      ],
      fills: [fill]
    }
    const structured = captureNode(square, new Map()) as { fills: unknown[] }
    expect(structured.fills).toEqual([{ type: 'SOLID', hex: '#161518', opacity: 1 }])
    expect(JSON.stringify(structured)).not.toContain('VariableID')
    const drawing = captureNode({ ...peak, fills: [fill] }, new Map([['peak', svg]]))
    expect(drawing).toMatchObject({ type: 'DRAWING', svg })
    expect(JSON.stringify(drawing)).not.toContain('VariableID')
  })

  it('should capture a rotated rectangle that paints nothing like a transparent node', () => {
    const empty = { ...peak, absoluteRenderBounds: null, fills: [] }
    expect(artworkIds(empty)).toEqual([])
    expect(captureNode(empty, new Map())).toMatchObject({ type: 'FRAME', opacity: 0 })
  })
})

describe('when artwork paints outside its node box', () => {
  const svg = '<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0"/></svg>'
  const ring = { type: 'SOLID', color: { r: 0.3, g: 0.08, b: 0.49, a: 1 } }
  // EventCard Community logo: a 36x36 frame whose ellipse has a 3px outside stroke.
  const logo: LiveNode = {
    id: 'logo',
    name: 'Frame 1',
    type: 'FRAME',
    absoluteBoundingBox: { x: 100, y: 200, width: 36, height: 36 },
    absoluteRenderBounds: { x: 98.5, y: 198.5, width: 39, height: 39 },
    children: [
      {
        id: 'ellipse',
        name: 'Ellipse',
        type: 'ELLIPSE',
        strokes: [ring],
        strokeWeight: 3,
        strokeAlign: 'OUTSIDE',
        absoluteBoundingBox: { x: 100, y: 200, width: 36, height: 36 },
        absoluteRenderBounds: { x: 98.5, y: 198.5, width: 39, height: 39 }
      }
    ]
  }
  // NotificationsPanel Media group: a 50x50 avatar whose badge overflows its top-right corner by 10px.
  const media: LiveNode = {
    id: 'media',
    name: 'Media',
    type: 'GROUP',
    absoluteBoundingBox: { x: 0, y: 0, width: 50, height: 50 },
    absoluteRenderBounds: { x: 0, y: -10, width: 60, height: 60 },
    children: [
      {
        id: 'avatar',
        name: 'Avatar',
        type: 'ELLIPSE',
        absoluteBoundingBox: { x: 0, y: 0, width: 50, height: 50 },
        absoluteRenderBounds: { x: 0, y: 0, width: 50, height: 50 }
      },
      {
        id: 'badge',
        name: 'Badge',
        type: 'ELLIPSE',
        absoluteBoundingBox: { x: 40, y: -10, width: 20, height: 20 },
        absoluteRenderBounds: { x: 40, y: -10, width: 20, height: 20 }
      }
    ]
  }

  it('should keep an outside stroke by exporting the render bounds around the layout box', () => {
    expect(artworkIds(logo)).toEqual(['logo'])
    expect(degenerateArtworkIds(logo)).toEqual(['logo'])
    expect(captureNode(logo, new Map([['logo', svg]]))).toEqual({
      type: 'FRAME',
      name: 'Frame 1',
      w: 36,
      h: 36,
      children: [
        {
          type: 'DRAWING',
          w: 39,
          h: 39,
          x: -1.5,
          y: -1.5,
          absolute: true,
          svg,
          sum: expect.any(String)
        }
      ]
    })
  })

  it('should keep a child that overflows its group', () => {
    expect(degenerateArtworkIds(media)).toEqual(['media'])
    expect(captureNode(media, new Map([['media', svg]]))).toMatchObject({
      type: 'FRAME',
      w: 50,
      h: 50,
      children: [{ type: 'DRAWING', w: 60, h: 60, x: 0, y: -10, absolute: true }]
    })
  })

  it('should place the layout box in its parent and offset only the drawing', () => {
    const parent: LiveNode = {
      id: 'card',
      name: 'Card',
      type: 'FRAME',
      absoluteBoundingBox: { x: 90, y: 190, width: 300, height: 120 },
      children: [logo]
    }
    expect(captureNode(logo, new Map([['logo', svg]]), parent)).toMatchObject({
      type: 'FRAME',
      x: 10,
      y: 10,
      w: 36,
      h: 36,
      children: [{ x: -1.5, y: -1.5, w: 39, h: 39 }]
    })
  })

  it.each([
    ['equal to its box', { x: 100, y: 200, width: 36, height: 36 }],
    ['inside its box', { x: 102, y: 202, width: 30, height: 30 }],
    ['past its box by less than the tolerance', { x: 99.995, y: 200, width: 36.01, height: 36 }]
  ])('should keep the box export when the render bounds are %s', (_, bounds) => {
    const node = { ...logo, absoluteRenderBounds: bounds }
    expect(degenerateArtworkIds(node)).toEqual([])
    expect(captureNode(node, new Map([['logo', svg]]))).toMatchObject({
      type: 'DRAWING',
      w: 36,
      h: 36,
      svg
    })
  })

  it('should refuse artwork whose render bounds are missing', () => {
    const missing: LiveNode = { ...logo }
    delete missing.absoluteRenderBounds
    expect(() => degenerateArtworkIds(missing)).toThrow('Missing render bounds: logo "Frame 1"')
    expect(() => captureNode(missing, new Map([['logo', svg]]))).toThrow('Missing render bounds')
  })

  it('should keep the box export for a mask Figma renders nothing for', () => {
    const mask = { ...logo, isMask: true, absoluteRenderBounds: null }
    expect(degenerateArtworkIds(mask)).toEqual([])
  })

  it('should refuse artwork whose render bounds are not numbers', () => {
    const broken = {
      ...logo,
      absoluteRenderBounds: { x: 98.5, y: Number.NaN, width: 39, height: 39 }
    }
    expect(() => degenerateArtworkIds(broken)).toThrow('Inconsistent render bounds: logo "Frame 1"')
  })

  describe.each([
    ['zero width', { x: 98.5, y: 198.5, width: 0, height: 39 }],
    ['negative height', { x: 98.5, y: 198.5, width: 39, height: -1 }]
  ])('and visible non-degenerate artwork has %s render bounds', (_, bounds) => {
    let broken: LiveNode

    beforeEach(() => {
      broken = { ...logo, absoluteRenderBounds: bounds }
    })

    it('should refuse export selection instead of choosing cropped bounds', () => {
      expect(() => degenerateArtworkIds(broken)).toThrow(
        'Inconsistent render bounds: logo "Frame 1"'
      )
    })

    it('should refuse capture even if an artwork export is supplied', () => {
      expect(() => captureNode(broken, new Map([['logo', svg]]))).toThrow(
        'Inconsistent render bounds: logo "Frame 1"'
      )
    })
  })
})
