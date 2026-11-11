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

const widths = (node: RawNode, env: ViewEnv) =>
  buildView(node, env)
    .children![0]!.css.decls.filter(([property]) => property === 'width')
    .map(([, value]) => value)

// A Search bar result row: a fixed-width, single-line label followed by the user count it pushes to the right.
const resultRow = (sizing: RawNode['sizing']): RawNode => ({
  type: 'FRAME',
  name: 'Frame 3104',
  w: 348,
  h: 36,
  layout: {
    mode: 'HORIZONTAL',
    pad: [0, 8, 0, 8],
    gap: 7,
    main: 'MIN',
    cross: 'CENTER',
    wrap: false
  },
  children: [
    {
      type: 'TEXT',
      name: 'Search item',
      text: 'Genesis Plaza',
      textStyle: 'body/sm',
      fontSize: 14,
      w: 238,
      h: 19,
      sizing,
      lineHeight: { unit: 'PIXELS', value: 19 }
    },
    { type: 'FRAME', name: 'Players', w: 34, h: 17, sizing: ['HUG', 'HUG'] }
  ]
})

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

  it('should keep a single line just under the two-line boundary on one line', () => {
    expect(whiteSpace(quest({ h: 20.9 }), env)).toEqual(['nowrap'])
    expect(env.report.manual.size).toBe(0)
  })

  it.each([
    ['just under two lines', 26.7],
    ['just over two lines', 29.3]
  ])('should count %s within the tolerance as two lines', (_, h) => {
    expect(whiteSpace(quest({ h }), env)).toEqual([])
    expect(env.report.manual.size).toBe(0)
  })

  it.each([
    ['below', 26.5],
    ['above', 29.5],
    ['well above, where the style line height would wrap', 34]
  ])('should not infer wrapping from a height %s the tolerance, and flag it instead', (_, h) => {
    expect(whiteSpace(quest({ h }), env)).toEqual(['nowrap'])
    expect([...env.report.manual]).toEqual([
      'text-wrap-ambiguous:Description: the height is not a whole number of captured lines, so it stays on one line'
    ])
  })

  // dcl-one keeps whole-line geometry meaningful in a fixed-height box (capturedLines in views.ts), where
  // figma-design-system flags it: two or three whole captured lines wrap, unflagged.
  it('should wrap a fixed-height box that holds a whole number of captured lines', () => {
    expect(whiteSpace(quest({ sizing: ['FILL', 'FIXED'] }), env)).not.toContain('nowrap')
    expect(whiteSpace(quest({ sizing: ['FILL', 'FIXED'], h: 42 }), env)).not.toContain('nowrap')
    expect(env.report.notes).not.toContain('text-wrap-ambiguous:Description')
  })

  it('should flag a fixed-height box even when it holds less than two lines', () => {
    expect(whiteSpace(quest({ sizing: ['FILL', 'FIXED'], h: 17 }), env)).toEqual(['nowrap'])
    expect(env.report.notes).toContain('text-wrap-ambiguous:Description')
  })

  it.each([
    ['exactly', 14, 14],
    ['within the tolerance of', 19, 19.4]
  ])('should keep a fixed-height box %s one line on one line, unflagged', (_, h, value) => {
    const node = quest({ sizing: ['FILL', 'FIXED'], h, lineHeight: { unit: 'PIXELS', value } })
    expect(whiteSpace(node, env)).toEqual(['nowrap'])
    expect(env.report.manual.size).toBe(0)
  })
})

describe('when a single-line text has a fixed width in an auto-layout row', () => {
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

  it('should keep its captured width, so the siblings after it sit where Figma puts them', () => {
    expect(widths(resultRow(['FIXED', 'HUG']), env)).toEqual(['238px'])
  })

  it('should keep a hugging text at its captured width', () => {
    expect(widths(resultRow(['HUG', 'HUG']), env)).toEqual(['238px'])
  })

  it('should let a filling text take the remaining space instead of a fixed width', () => {
    expect(widths(resultRow(['FILL', 'HUG']), env)).toEqual([])
  })
})
