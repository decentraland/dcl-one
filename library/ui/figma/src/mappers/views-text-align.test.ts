import { beforeEach, describe, expect, it } from 'vitest'
import type { RawNode } from '../ir/raw-components.js'
import type { ColorResolver } from './colors.js'
import { buildView, type ViewEnv } from './views.js'

// The Nav bar / Market tab: a 140 px wide centered label placed freely under its icon.
const tab = (label: Partial<RawNode>, layout?: RawNode['layout']): RawNode => ({
  type: 'COMPONENT',
  name: 'Stat=off',
  w: 140,
  h: 58,
  ...(layout ? { layout } : {}),
  children: [
    {
      type: 'TEXT',
      name: 'shop [-]',
      text: 'shop [-] ',
      fontSize: 13,
      w: 140,
      h: 17,
      x: 0,
      y: 36,
      textAlignHorizontal: 'CENTER',
      lineHeight: { unit: 'PIXELS', value: 15.73 },
      ...label
    }
  ]
})

const row: RawNode['layout'] = {
  mode: 'HORIZONTAL',
  pad: [0, 0, 0, 0],
  gap: 0,
  main: 'MIN',
  cross: 'MIN',
  wrap: false
}

const widths = (node: RawNode, env: ViewEnv) =>
  buildView(node, env)
    .children![0]!.css.decls.filter(([property]) => property === 'width')
    .map(([, value]) => value)

describe('when a single-line text node aligns within its box', () => {
  let env: ViewEnv
  beforeEach(() => {
    env = {
      colors: { resolve: () => ({ css: '#ffffff', via: 'literal' }) } as unknown as ColorResolver,
      textStyles: new Map(),
      textStyleAliases: {},
      numbers: { space: new Map(), radius: new Map() },
      icons: new Map(),
      iconAliases: {},
      refsById: new Map(),
      report: { notes: new Set(), manual: new Set() }
    }
  })

  it.each(['CENTER', 'RIGHT', 'JUSTIFIED'])(
    'should keep the captured width of a freely placed %s-aligned label',
    (align) => {
      expect(widths(tab({ textAlignHorizontal: align }), env)).toEqual(['140px'])
    }
  )

  it('should keep the width once for a label that is both fixed and centered', () => {
    expect(widths(tab({ sizing: ['FIXED', 'HUG'] }, row), env)).toEqual(['140px'])
  })

  it('should keep the width of a fixed left-aligned label for its fixed box alone', () => {
    expect(
      widths(tab({ sizing: ['FIXED', 'HUG'], textAlignHorizontal: 'LEFT' }, row), env)
    ).toEqual(['140px'])
  })

  it('should keep only the hug width of a centered hugging label', () => {
    expect(widths(tab({ sizing: ['HUG', 'HUG'], w: 64 }, row), env)).toEqual(['64px'])
  })

  it('should leave a filling centered label to the flex layout', () => {
    expect(widths(tab({ sizing: ['FILL', 'HUG'] }, row), env)).toEqual([])
  })

  it.each([
    ['left-aligned', { textAlignHorizontal: 'LEFT' }],
    ['unaligned', { textAlignHorizontal: undefined }]
  ])('should not size a freely placed %s label', (_, label) => {
    expect(widths(tab(label), env)).toEqual([])
  })
})
