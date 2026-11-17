import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { RawNode } from '../ir/raw-components.js'
import type { ColorResolver } from './colors.js'
import { buildView, type ViewEnv } from './views.js'

type View = ReturnType<typeof buildView>
const decl = (view: View | undefined, property: string) =>
  view?.css.decls.find(([key]) => key === property)?.[1]

// The mobile NavBar: a 390px row whose space-between spacing still stores 854.67px from the desktop frame.
const logo: RawNode = { type: 'FRAME', name: 'DCL Logo', w: 48, h: 48, children: [] }
const signIn: RawNode = { type: 'FRAME', name: 'Sign In / Profile', w: 104, h: 46, children: [] }
const bar = (layout: Partial<NonNullable<RawNode['layout']>>, children: RawNode[]): RawNode => ({
  type: 'FRAME',
  name: 'viewport=mobile, signed-in=false',
  w: 390,
  h: 72,
  layout: {
    mode: 'HORIZONTAL',
    pad: [12, 16, 12, 16],
    gap: 854.67,
    main: 'SPACE_BETWEEN',
    cross: 'MIN',
    ...layout
  },
  children
})

describe('when an auto-layout frame distributes its children with space-between', () => {
  let env: ViewEnv
  beforeEach(() => {
    env = {
      colors: { resolve: vi.fn(() => ({ css: '#161518' })) } as unknown as ColorResolver,
      textStyles: new Map(),
      textStyleAliases: {},
      numbers: { space: new Map([[8, '--dcl-space-8']]), radius: new Map() },
      icons: new Map(),
      iconAliases: {},
      refsById: new Map(),
      report: { notes: new Set(), manual: new Set() }
    }
  })

  it('should drop the stale item spacing of a row so the trailing child stays inside the frame', () => {
    const view = buildView(bar({}, [logo, signIn]), env)

    expect(decl(view, 'justify-content')).toBe('space-between')
    expect(decl(view, 'gap')).toBeUndefined()
  })

  it('should drop the stale item spacing of a column', () => {
    const view = buildView(bar({ mode: 'VERTICAL' }, [logo, signIn]), env)

    expect(decl(view, 'flex-direction')).toBe('column')
    expect(decl(view, 'gap')).toBeUndefined()
  })

  it('should not overlap the children when the stored spacing is negative', () => {
    const view = buildView(bar({ gap: -8 }, [logo, signIn]), env)

    expect(view.children?.map((child) => decl(child, 'margin-left'))).toEqual([
      undefined,
      undefined
    ])
  })

  it('should still overlap the children of a packed frame with negative spacing', () => {
    const view = buildView(bar({ main: 'MIN', gap: -8 }, [logo, signIn]), env)

    expect(view.children?.map((child) => decl(child, 'margin-left'))).toEqual([
      undefined,
      'calc(var(--dcl-space-8) * -1)'
    ])
  })

  it('should keep the gap of a packed frame whose cross axis is not space-between', () => {
    const view = buildView(bar({ main: 'MIN', cross: 'CENTER', gap: 8 }, [logo, signIn]), env)

    expect(decl(view, 'gap')).toBe('var(--dcl-space-8)')
  })

  // Wrapped track spacing is counterAxisSpacing, which the capture does not record yet.
  it('should leave a wrapped space-between frame as it was', () => {
    const view = buildView(bar({ gap: 8, wrap: true }, [logo, signIn]), env)

    expect(view.css.decls).toEqual([
      ['display', 'flex'],
      ['box-sizing', 'border-box'],
      ['flex-direction', 'row'],
      ['justify-content', 'space-between'],
      ['align-items', 'flex-start'],
      ['flex-wrap', 'wrap'],
      ['gap', 'var(--dcl-space-8)'],
      ['padding', '12px 16px 12px 16px'],
      ['width', '390px'],
      ['height', '72px']
    ])
  })
})
