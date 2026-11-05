import { describe, expect, it } from 'vitest'
import { parseRows, splitFrame, toRecord } from './split-icon-frames.js'

const rowsText = [
  '## 1:1|Nav bar',
  'navbar/wallet/active=false|2:1|20|20|36|36',
  'navbar/wallet/active=true|2:2|20|106|36|36',
  '## 1:2|General',
  'general/like|3:1|88|0|24|24'
].join('\n')

const frame = `<svg width="200" height="200" viewBox="0 0 200 200" fill="none" xmlns="http://www.w3.org/2000/svg">
<g id="Nav bar &#38; Side bar">
<g id="navbar/wallet">
<g id="active=false"><path id="Vector" d="M0 0h1" fill="url(#g1)"/></g>
<g id="active=true"><path id="Vector_2" d="M0 0h2" stroke="#fff"/></g>
</g>
</g>
<defs>
<linearGradient id="g1"><stop stop-color="#fff"/></linearGradient>
<linearGradient id="unused"><stop stop-color="#000"/></linearGradient>
</defs>
</svg>`

describe('parseRows', () => {
  describe('when a row ends with a variant segment', () => {
    it('should use the variant as the component name to match in the export', () => {
      const [nav, general] = parseRows(rowsText)

      expect(nav?.rows.map((r) => r.componentName)).toEqual(['active=false', 'active=true'])
      expect(general?.rows[0]?.componentName).toBe('general/like')
    })
  })
})

describe('splitFrame', () => {
  const [nav] = parseRows(rowsText)

  describe('when the export nests variants inside a wrapper group', () => {
    it('should extract one SVG per row with a viewBox in frame coordinates', () => {
      const { icons, missing } = splitFrame(frame, nav!.rows)

      expect(missing).toEqual([])
      expect(icons.map((i) => i.svg.match(/viewBox="([^"]+)"/)?.[1])).toEqual([
        '20 20 36 36',
        '20 106 36 36'
      ])
    })

    it('should carry only the defs an icon references', () => {
      const [first, second] = splitFrame(frame, nav!.rows).icons

      expect(first?.svg).toContain('id="g1"')
      expect(first?.svg).not.toContain('id="unused"')
      expect(second?.svg).not.toContain('<defs>')
    })
  })

  describe('when a row has no matching group in the export', () => {
    it('should report it as missing', () => {
      const { missing } = splitFrame(frame, [
        ...nav!.rows,
        { ...nav!.rows[0]!, path: 'x/y', componentName: 'x/y' }
      ])

      expect(missing).toEqual(['x/y'])
    })
  })
})

describe('toRecord', () => {
  describe('when the row is a variant', () => {
    it('should split name and variant and derive a file-safe name', () => {
      const [nav] = parseRows(rowsText)

      expect(toRecord(nav!.rows[0]!, 'Nav bar')).toMatchObject({
        name: 'navbar/wallet',
        variant: 'active=false',
        category: 'navbar',
        file: 'navbar__wallet__active-false.svg'
      })
    })
  })
})
