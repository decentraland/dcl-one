import { describe, expect, it } from 'vitest'
import type { TokenCollection } from '../ir/schemas.js'
import { readSwatches } from './color-swatches.js'
import type { LiveNode } from './live-file.js'

const rgb = (hex: string) => ({
  r: parseInt(hex.slice(1, 3), 16) / 255,
  g: parseInt(hex.slice(3, 5), 16) / 255,
  b: parseInt(hex.slice(5, 7), 16) / 255,
  a: 1
})

let id = 0
function swatch(label: string, value: string, fill = value.slice(0, 7), opacity = 1): LiveNode {
  id += 1
  return {
    id: `s:${id}`,
    name: 'Swatch',
    type: 'INSTANCE',
    children: [
      {
        id: `r:${id}`,
        name: 'Rectangle 1',
        type: 'RECTANGLE',
        fills: [{ type: 'SOLID', color: rgb(fill), opacity }]
      },
      { id: `l:${id}`, name: 'label', type: 'TEXT', characters: label },
      { id: `v:${id}`, name: 'value', type: 'TEXT', characters: value }
    ]
  }
}
const column = (name: string, ...children: LiveNode[]): LiveNode => ({
  id: `c:${name}`,
  name,
  type: 'FRAME',
  children: [{ id: `t:${name}`, name: 'title', type: 'TEXT', characters: name }, ...children]
})
const page = (...children: LiveNode[]): LiveNode => ({
  id: 'page',
  name: 'Colors',
  type: 'CANVAS',
  children
})
const primitives = (values: Record<string, [string, number?]>): TokenCollection[] => [
  {
    name: 'Primitives',
    modes: ['Value'],
    variables: Object.entries(values).map(([name, [hex, alpha = 1]]) => ({
      name,
      type: 'COLOR' as const,
      values: { Value: { hex, alpha } }
    }))
  }
]
const saved = primitives({
  'Primary/Primary': ['#ff2d55'],
  'Primary/Primary Dark 1': ['#f70038'],
  'System/Orange': ['#fe9c2a'],
  'System/Orange Dark 1': ['#f38025'],
  'Brand/Orange': ['#ff7439'],
  'Brand/Yellow': ['#ffc95b'],
  'Transparencies/Black 60': ['#000000', 0.6000000238418579]
})

describe('readSwatches', () => {
  describe('when every swatch matches its saved primitive', () => {
    it('should report no changes', () => {
      const result = readSwatches(
        page(
          column('Reds', swatch('Primary', '#FF2D55'), swatch('Primary Dark 1', '#F70038')),
          column('Warnings', swatch('Orange', '#FE9C2A'), swatch('Orange Dark 1', '#F38025')),
          column('Brand', swatch('Orange', '#FF7439'), swatch('Yellow', '#FFC95B')),
          column('Black', swatch('Backdrop', '#000000 - 60%', '#000000', 0.6))
        ),
        saved
      )

      expect(result).toMatchObject({ changes: [], errors: [] })
    })
  })

  describe('when a swatch shows a new value for a saved primitive', () => {
    it('should update the primitive of its column group', () => {
      const result = readSwatches(
        page(
          column('Brand', swatch('Orange', '#FF7439'), swatch('Yellow', '#FFBC5B')),
          column('Warnings', swatch('Orange', '#FE9C2A'), swatch('Orange Dark 1', '#F38025'))
        ),
        saved
      )

      expect(result.changes).toEqual([
        {
          kind: 'updated',
          name: 'Brand/Yellow',
          from: '#ffc95b',
          to: '#ffbc5b',
          fromAlpha: 1,
          toAlpha: 1
        }
      ])
      expect(
        result.collections[0]!.variables.find((variable) => variable.name === 'Brand/Yellow')
      ).toMatchObject({ values: { Value: { hex: '#ffbc5b', alpha: 1 } } })
    })
  })

  describe('when only the opacity of a swatch changes', () => {
    it('should report the alpha before and after, since the hex is the same', () => {
      const result = readSwatches(
        page(
          column(
            'Brand',
            swatch('Orange', '#FF7439'),
            swatch('Yellow', '#FFC95B - 50%', '#FFC95B', 0.5)
          )
        ),
        saved
      )

      expect(result.changes).toEqual([
        {
          kind: 'updated',
          name: 'Brand/Yellow',
          from: '#ffc95b',
          to: '#ffc95b',
          fromAlpha: 1,
          toAlpha: 0.5
        }
      ])
    })
  })

  describe('when a column gains a swatch with a new label', () => {
    it('should add a primitive in the group of that column', () => {
      const result = readSwatches(
        page(column('Reds', swatch('Primary', '#FF2D55'), swatch('Primary blue TEST', '#0400FF'))),
        saved
      )

      expect(result.changes).toEqual([
        { kind: 'added', name: 'Primary/Primary blue TEST', to: '#0400ff', toAlpha: 1 }
      ])
    })
  })

  describe('when a swatch shows an excluded primitive', () => {
    const withTest = primitives({
      'Primary/Primary': ['#ff2d55'],
      'Primary/Primary blue TEST': ['#0400ff']
    })
    const excluded = ['Primitives/Primary/Primary blue TEST']

    it('should report it as excluded and keep the saved variable', () => {
      const result = readSwatches(
        page(column('Reds', swatch('Primary', '#FF2D55'), swatch('Primary blue TEST', '#0400FF'))),
        withTest,
        excluded
      )

      expect(result.changes).toEqual([{ kind: 'excluded', name: 'Primary/Primary blue TEST' }])
      expect(result.collections).toEqual(withTest)
      expect(result.errors).toEqual([])
    })

    it('should not take a new value from the swatch', () => {
      const result = readSwatches(
        page(column('Reds', swatch('Primary', '#FF2D55'), swatch('Primary blue TEST', '#00FF00'))),
        withTest,
        excluded
      )

      expect(result.changes).toEqual([{ kind: 'excluded', name: 'Primary/Primary blue TEST' }])
      expect(result.collections).toEqual(withTest)
    })

    it('should still update a sibling swatch in the same column', () => {
      const result = readSwatches(
        page(column('Reds', swatch('Primary', '#FF0000'), swatch('Primary blue TEST', '#0400FF'))),
        withTest,
        excluded
      )

      expect(result.changes).toEqual([
        {
          kind: 'updated',
          name: 'Primary/Primary',
          from: '#ff2d55',
          to: '#ff0000',
          fromAlpha: 1,
          toAlpha: 1
        },
        { kind: 'excluded', name: 'Primary/Primary blue TEST' }
      ])
      expect(
        result.collections[0]!.variables.find((variable) => variable.name === 'Primary/Primary')
      ).toMatchObject({ values: { Value: { hex: '#ff0000', alpha: 1 } } })
    })

    it('should still report an unrelated malformed swatch', () => {
      const result = readSwatches(
        page(
          column(
            'Reds',
            swatch('Primary', '#FF2D55', '#00FF00'),
            swatch('Primary blue TEST', '#0400FF')
          )
        ),
        withTest,
        excluded
      )

      expect(result.errors).toEqual([
        expect.stringMatching(
          /^Swatch "Primary" \(s:\d+\) says #FF2D55 but is filled with #00ff00 at 1$/
        )
      ])
    })

    it('should still report a malformed swatch of the excluded primitive itself', () => {
      const result = readSwatches(
        page(
          column(
            'Reds',
            swatch('Primary', '#FF2D55'),
            swatch('Primary blue TEST', '#0400FF', '#00FF00')
          )
        ),
        withTest,
        excluded
      )

      expect(result.errors).toEqual([
        expect.stringMatching(
          /^Swatch "Primary blue TEST" \(s:\d+\) says #0400FF but is filled with #00ff00 at 1$/
        )
      ])
    })

    it('should report it once, without a kept note, when its swatch is missing', () => {
      const result = readSwatches(
        page(column('Reds', swatch('Primary', '#FF2D55'))),
        withTest,
        excluded
      )

      expect(result.changes).toEqual([{ kind: 'excluded', name: 'Primary/Primary blue TEST' }])
      expect(result.notes).toEqual([])
      expect(result.collections).toEqual(withTest)
    })

    it('should report it once when two swatches show it', () => {
      const result = readSwatches(
        page(
          column(
            'Reds',
            swatch('Primary', '#FF2D55'),
            swatch('Primary blue TEST', '#0400FF'),
            swatch('Primary blue TEST', '#0400FF')
          )
        ),
        withTest,
        excluded
      )

      expect(result.changes).toEqual([{ kind: 'excluded', name: 'Primary/Primary blue TEST' }])
    })

    it('should not add it when the capture does not have it yet', () => {
      const result = readSwatches(
        page(column('Reds', swatch('Primary', '#FF2D55'), swatch('Primary blue TEST', '#0400FF'))),
        saved,
        excluded
      )

      expect(result.changes).toEqual([{ kind: 'excluded', name: 'Primary/Primary blue TEST' }])
      expect(result.collections).toEqual(saved)
    })
  })

  describe('when the hex text disagrees with the swatch fill', () => {
    it('should report the swatch instead of reading either value', () => {
      const result = readSwatches(
        page(column('Brand', swatch('Orange', '#FF7439'), swatch('Yellow', '#FFBC5B', '#FFC95B'))),
        saved
      )

      expect(result.changes).toEqual([])
      expect(result.errors).toEqual([
        expect.stringMatching(
          /^Swatch "Yellow" \(s:\d+\) says #FFBC5B but is filled with #ffc95b at 1$/
        )
      ])
    })
  })

  describe('when a transparency swatch paints its colour over a checkerboard rectangle', () => {
    it('should read the colour of the topmost rectangle', () => {
      const transparent = swatch('Backdrop', '#000000 - 60%', '#000000', 0.6)
      transparent.children!.unshift({
        id: 'checkerboard',
        name: 'transparent-bg',
        type: 'RECTANGLE',
        fills: [{ type: 'IMAGE' }]
      })

      expect(readSwatches(page(column('Black', transparent)), saved)).toMatchObject({
        changes: [],
        errors: []
      })
    })
  })

  describe('when an image is painted over the swatch colour', () => {
    it('should report the swatch instead of reading the colour underneath', () => {
      const covered = swatch('Yellow', '#FFC95B')
      covered.children![0]!.fills = [{ type: 'SOLID', color: rgb('#ffc95b') }, { type: 'IMAGE' }]

      expect(
        readSwatches(page(column('Brand', swatch('Orange', '#FF7439'), covered)), saved).errors
      ).toEqual([
        expect.stringMatching(
          /^Swatch "Yellow" \(s:\d+\) says #FFC95B but is filled with no solid colour$/
        )
      ])
    })
  })

  describe('when a new label repeats a value of its group or a primitive has no swatch', () => {
    it('should keep the primitives and list both in the notes', () => {
      const result = readSwatches(
        page(column('Brand', swatch('Yellow', '#FFC95B'), swatch('Gold', '#FFC95B'))),
        saved
      )

      expect(result.changes).toEqual([])
      expect(result.notes).toEqual([
        expect.stringMatching(
          /^Swatch "Gold" \(s:\d+\) shows the value of Brand\/Yellow and was not added$/
        ),
        'Brand/Orange has no swatch on the Colors page and was kept'
      ])
    })

    it('should list a primitive of a shown group that lost its swatch', () => {
      const result = readSwatches(page(column('Brand', swatch('Yellow', '#FFC95B'))), saved)

      expect(result.notes).toEqual(['Brand/Orange has no swatch on the Colors page and was kept'])
    })
  })

  describe('when a swatch describes a gradient instead of a hex value', () => {
    it('should skip it', () => {
      const gradient = swatch('Rainbow', 'All brand colours', '#ffffff')

      expect(readSwatches(page(column('Gradients', gradient)), saved)).toMatchObject({
        changes: [],
        errors: []
      })
    })
  })
})
