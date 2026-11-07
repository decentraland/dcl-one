import { beforeEach, describe, expect, it } from 'vitest'
import type { RawNode } from '../ir/raw-components.js'
import type { ColorResolver } from './colors.js'
import { buildView, type ViewEnv } from './views.js'

// The NotificationsPanel quest text: body/sm (14 px at 150%, so 21 px) with a captured 14 px line height.
const quest = (overrides: Partial<RawNode>): RawNode => ({
  type: 'FRAME',
  name: 'Content',
  w: 266,
  h: 60,
  layout: { mode: 'VERTICAL', pad: [0, 0, 0, 0], gap: 0, main: 'MIN', cross: 'MIN', wrap: false },
  children: [
    {
      type: 'TEXT',
      name: 'Description',
      text: 'Long hair is already in your Backpack.',
      textStyle: 'body/sm',
      fontSize: 14,
      w: 266,
      h: 28,
      sizing: ['FILL', 'HUG'],
      lineHeight: { unit: 'PIXELS', value: 14 },
      ...overrides
    }
  ]
})

const whiteSpace = (node: RawNode, env: ViewEnv) =>
  buildView(node, env)
    .children![0]!.css.decls.filter(([property]) => property === 'white-space')
    .map(([, value]) => value)

describe('when a text node carries its own line height', () => {
  let env: ViewEnv
  beforeEach(() => {
    env = {
      colors: { resolve: () => ({ css: '#ffffff', via: 'literal' }) } as unknown as ColorResolver,
      textStyles: new Map([
        [
          'body/sm',
          {
            name: 'body/sm',
            fontFamily: 'Inter',
            fontStyle: 'Regular',
            fontSize: 14,
            lineHeightPercent: 150,
            letterSpacingPx: 0,
            textCase: 'ORIGINAL'
          }
        ]
      ]),
      textStyleAliases: {},
      numbers: { space: new Map(), radius: new Map() },
      icons: new Map(),
      iconAliases: {},
      refsById: new Map(),
      report: { notes: new Set(), manual: new Set() }
    }
  })

  it('should let two captured lines wrap, though the style line height would call them one', () => {
    expect(whiteSpace(quest({}), env)).toEqual([])
  })

  it('should keep a single captured line on one line', () => {
    expect(whiteSpace(quest({ h: 17 }), env)).toEqual(['nowrap'])
  })

  it('should read a percent line height against the node font size', () => {
    expect(whiteSpace(quest({ lineHeight: { unit: 'PERCENT', value: 100 } }), env)).toEqual([])
  })

  it('should fall back to the style line height when the node has none', () => {
    expect(whiteSpace(quest({ lineHeight: undefined }), env)).toEqual(['nowrap'])
    expect(whiteSpace(quest({ lineHeight: undefined, h: 42 }), env)).toEqual([])
  })

  it('should fall back to the style line height for an auto line height', () => {
    expect(whiteSpace(quest({ lineHeight: { unit: 'AUTO' } }), env)).toEqual(['nowrap'])
  })
})
