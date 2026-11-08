import { beforeEach, describe, expect, it } from 'vitest'
import type { RawNode } from '../ir/raw-components.js'
import type { ColorResolver } from './colors.js'
import { buildView, type ViewEnv } from './views.js'

describe('when mapping captured text geometry and overlapping layers', () => {
  let env: ViewEnv
  beforeEach(() => {
    env = {
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

  it.each(['PIXELS', 'PERCENT'] as const)(
    'should wrap a fixed box holding exactly two %s lines',
    (unit) => {
      const view = buildView(
        {
          type: 'TEXT',
          text: 'two captured lines',
          w: 80,
          h: 48,
          sizing: ['FIXED', 'FIXED'],
          fontSize: 16,
          lineHeight: { unit, value: unit === 'PIXELS' ? 24 : 150 }
        } as RawNode,
        env
      )
      expect(view.css.decls).not.toContainEqual(['white-space', 'nowrap'])
      expect(env.report.manual.size).toBe(0)
    }
  )

  it.each([
    ['single', 24, 'FIXED', true],
    ['hugging short', 29, 'HUG', true],
    ['hugging multiple', 72, 'HUG', false]
  ] as const)('should preserve %s captured line geometry', (_, height, sizing, nowrap) => {
    const view = buildView(
      {
        type: 'TEXT',
        text: 'captured',
        w: 80,
        h: height,
        sizing: ['FIXED', sizing],
        lineHeight: { unit: 'PIXELS', value: 24 }
      } as RawNode,
      env
    )
    expect(view.css.decls.some(([key, value]) => key === 'white-space' && value === 'nowrap')).toBe(
      nowrap
    )
  })

  it('should report a genuinely fractional captured line count', () => {
    buildView(
      {
        type: 'TEXT',
        text: 'uncertain',
        w: 80,
        h: 36,
        sizing: ['FIXED', 'FIXED'],
        lineHeight: { unit: 'PIXELS', value: 24 }
      } as RawNode,
      env
    )
    expect([...env.report.manual]).toContainEqual(expect.stringContaining('text-wrap-ambiguous'))
  })

  it('should keep HUG text shrinkable in an auto-layout row', () => {
    const view = buildView(
      {
        type: 'FRAME',
        w: 40,
        h: 24,
        layout: { mode: 'HORIZONTAL', pad: [0, 0, 0, 0], gap: 0, main: 'MIN', cross: 'MIN' },
        children: [{ type: 'TEXT', text: 'long label', w: 100, h: 24, sizing: ['HUG', 'HUG'] }]
      } as RawNode,
      env
    )
    expect(view.children![0]!.css.decls).not.toContainEqual(['flex-shrink', '0'])
  })

  it.each([
    ['SPACE_BETWEEN', false],
    ['MIN', true]
  ])(
    'should emit the item spacing as a CSS gap only when the row is not %s-distributed',
    (main, gap) => {
      // Figma ignores itemSpacing under SPACE_BETWEEN; a CSS gap would be a minimum that overflows a narrower box.
      const view = buildView(
        {
          type: 'FRAME',
          w: 390,
          h: 48,
          layout: { mode: 'HORIZONTAL', pad: [0, 0, 0, 0], gap: 854.67, main, cross: 'CENTER' },
          children: [
            { type: 'FRAME', name: 'logo', w: 32, h: 32 },
            { type: 'FRAME', name: 'menu', w: 32, h: 32 }
          ]
        } as RawNode,
        env
      )
      expect(view.css.decls.some(([property]) => property === 'gap')).toBe(gap)
      expect(view.css.decls).toContainEqual([
        'justify-content',
        main === 'SPACE_BETWEEN' ? 'space-between' : 'flex-start'
      ])
    }
  )

  it('should ignore a leading mask for spacing and paint a later absolute overlay on top', () => {
    const view = buildView(
      {
        type: 'FRAME',
        w: 100,
        h: 24,
        layout: { mode: 'HORIZONTAL', pad: [0, 0, 0, 0], gap: -8, main: 'MIN', cross: 'MIN' },
        children: [
          { type: 'FRAME', name: 'mask', w: 100, h: 24, isMask: true },
          { type: 'FRAME', name: 'first', w: 24, h: 24 },
          { type: 'FRAME', name: 'second', w: 24, h: 24 },
          { type: 'FRAME', name: '+3', w: 24, h: 24, absolute: true, x: 30, y: 0 }
        ]
      } as RawNode,
      env
    )
    expect(view.children![0]!.css.decls).not.toContainEqual(['margin-left', '-8px'])
    expect(view.children![1]!.css.decls).toContainEqual(['margin-left', '-8px'])
    expect(view.children![2]!.css.decls).toContainEqual(['z-index', '3'])
  })
})
