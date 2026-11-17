import { describe, expect, it } from 'vitest'
import {
  collectIconComponents,
  mapRestGradient,
  mapRestTextStyle,
  mapRestVariables,
  type RestNode
} from './rest-mappers.js'

describe('mapRestVariables', () => {
  const meta = {
    variableCollections: {
      c1: { name: 'Primitives', modes: [{ modeId: 'm1', name: 'Value' }], variableIds: ['v1'] },
      c2: { name: 'Semantic', modes: [{ modeId: 'm2', name: 'Dark' }], variableIds: ['v2', 'v3'] }
    },
    variables: {
      v1: {
        name: 'Neutrals/Shadow',
        resolvedType: 'COLOR',
        variableCollectionId: 'c1',
        valuesByMode: { m1: { r: 0.0863, g: 0.0824, b: 0.0941, a: 1 } }
      },
      v2: {
        name: 'bg/page',
        resolvedType: 'COLOR',
        variableCollectionId: 'c2',
        valuesByMode: { m2: { type: 'VARIABLE_ALIAS', id: 'v1' } }
      },
      v3: {
        name: 'label',
        resolvedType: 'STRING',
        variableCollectionId: 'c2',
        valuesByMode: { m2: 'x' }
      }
    }
  }

  describe('when a variable aliases another collection', () => {
    it('should map the alias by name and collection', () => {
      const [, semantic] = mapRestVariables(meta)

      expect(semantic?.variables[0]?.values).toEqual({
        Dark: { alias: 'Neutrals/Shadow', collection: 'Primitives' }
      })
    })
  })

  describe('when a variable is a color', () => {
    it('should convert channels to a lowercase hex with alpha', () => {
      const [primitives] = mapRestVariables(meta)

      expect(primitives?.variables[0]?.values.Value).toEqual({ hex: '#161518', alpha: 1 })
    })
  })

  describe('when a variable is neither COLOR nor FLOAT', () => {
    it('should skip it', () => {
      const [, semantic] = mapRestVariables(meta)

      expect(semantic?.variables.map((v) => v.name)).toEqual(['bg/page'])
    })
  })
})

describe('mapRestTextStyle', () => {
  describe('when the weight is 600 and the case is UPPER', () => {
    it('should map to Semi Bold and UPPER', () => {
      const style = mapRestTextStyle('label/md-caps', {
        fontFamily: 'Inter',
        fontWeight: 600,
        fontSize: 16,
        lineHeightPercentFontSize: 125,
        letterSpacing: 0.5,
        textCase: 'UPPER'
      })

      expect(style).toMatchObject({
        fontStyle: 'Semi Bold',
        textCase: 'UPPER',
        lineHeightPercent: 125
      })
    })
  })

  describe('when optional fields are missing', () => {
    it('should fall back to Regular, ORIGINAL and zero spacing', () => {
      const style = mapRestTextStyle('body/md', {
        fontFamily: 'Inter',
        fontWeight: 123,
        fontSize: 16
      })

      expect(style).toMatchObject({
        fontStyle: 'Regular',
        textCase: 'ORIGINAL',
        letterSpacingPx: 0
      })
    })
  })
})

describe('mapRestGradient', () => {
  const stops = [
    { position: 0, color: { r: 1, g: 0, b: 0 } },
    { position: 1, color: { r: 0, g: 0, b: 1 } }
  ]

  describe('when the handles point to the right', () => {
    it('should resolve a 90deg linear gradient', () => {
      const gradient = mapRestGradient('Gradients/Rainbow', {
        type: 'GRADIENT_LINEAR',
        gradientHandlePositions: [
          { x: 0, y: 0.5 },
          { x: 1, y: 0.5 }
        ],
        gradientStops: stops
      })

      expect(gradient).toMatchObject({ type: 'linear', angleDeg: 90 })
    })
  })

  describe('when the paint is not a gradient', () => {
    it('should return undefined', () => {
      expect(mapRestGradient('x', { type: 'SOLID', gradientStops: stops })).toBeUndefined()
    })
  })
})

describe('collectIconComponents', () => {
  const section: RestNode = {
    id: '1',
    name: 'Nav bar',
    type: 'FRAME',
    children: [
      {
        id: '2',
        name: 'Frame',
        type: 'FRAME',
        children: [
          {
            id: '3',
            name: 'navbar/credits',
            type: 'COMPONENT',
            absoluteBoundingBox: { x: 0, y: 0, width: 36, height: 36 }
          },
          {
            id: '4',
            name: 'navbar/wallet',
            type: 'FRAME',
            children: [
              { id: '5', name: 'active=false', type: 'COMPONENT' },
              { id: '6', name: 'active=true', type: 'COMPONENT', visible: false }
            ]
          }
        ]
      }
    ]
  }

  describe('when a frame wraps variant components', () => {
    it('should name the icon after the wrapper and keep the variant', () => {
      const found = collectIconComponents(section)

      expect(found.map((c) => [c.path, c.name, c.variant])).toEqual([
        ['navbar/credits', 'navbar/credits', null],
        ['navbar/wallet/active=false', 'navbar/wallet', 'active=false']
      ])
    })

    it('should skip hidden components', () => {
      expect(collectIconComponents(section).some((c) => c.nodeId === '6')).toBe(false)
    })
  })
})
