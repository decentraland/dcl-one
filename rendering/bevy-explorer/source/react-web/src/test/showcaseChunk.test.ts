import { describe, expect, it } from 'vitest'
import { designSideEffects, showcaseOnly } from '../../vite.config'

// The build's chunking of the design modules: what only the dev-only Showcase uses stays out of the eager chunks.
const DESIGN = '/repo/library/ui/apps/hud/design'
const ENTRY = `${DESIGN}/index.ts`
const SHOWCASE = `${DESIGN}/Showcase.tsx`
const SHOP = '/repo/library/ui/apps/hud/features/shop/ShopPage.tsx'

type Module = { importers: string[]; exports: string[]; code: string | null }

// A design entry re-exporting DiscoverCard, which only the Showcase names, and Button, which the Shop names too.
function graph(changes: Record<string, Partial<Module>> = {}) {
  const modules: Record<string, Partial<Module>> = {
    [ENTRY]: { importers: [SHOWCASE, SHOP] },
    [SHOWCASE]: { code: 'import { Button, DiscoverCard } from "."' },
    [SHOP]: { code: 'import { Button } from "../../design"; jsx(Button, {})' },
    [`${DESIGN}/DiscoverCard.tsx`]: { importers: [ENTRY], exports: ['DiscoverCard', 'DiscoverGrid'] },
    [`${DESIGN}/DiscoverCard.module.css`]: { importers: [`${DESIGN}/DiscoverCard.tsx`], exports: ['default'] },
    [`${DESIGN}/Button.tsx`]: { importers: [ENTRY], exports: ['Button'] },
    [`${DESIGN}/Button.module.css`]: { importers: [`${DESIGN}/Button.tsx`], exports: ['default'] },
    ...changes
  }
  return {
    getModuleInfo: (id: string): Module | null =>
      id in modules ? { importers: [], exports: [], code: '', ...modules[id] } : null
  }
}

describe('when the build places the design modules', () => {
  it('should keep a primitive only the Showcase names, and its styles, with the Showcase', () => {
    const only = showcaseOnly(graph())
    expect(only(`${DESIGN}/DiscoverCard.tsx`)).toBe(true)
    expect(only(`${DESIGN}/DiscoverCard.module.css`)).toBe(true)
  })

  it('should leave a primitive another importer of the design entry names in place, with its styles', () => {
    const only = showcaseOnly(graph())
    expect(only(`${DESIGN}/Button.tsx`)).toBe(false)
    expect(only(`${DESIGN}/Button.module.css`)).toBe(false)
  })

  it('should leave the design entry, the Showcase and modules outside design/ in place', () => {
    const only = showcaseOnly(graph())
    for (const id of [ENTRY, SHOWCASE, `${DESIGN}/ShowcaseIndex.ts`, SHOP]) expect(only(id)).toBe(false)
  })

  it('should leave a primitive in place when an importer of the design entry has no code to read', () => {
    const only = showcaseOnly(graph({ [SHOP]: { code: null } }))
    expect(only(`${DESIGN}/DiscoverCard.tsx`)).toBe(false)
  })

  it('should leave a primitive a module outside design/ imports directly in place', () => {
    const only = showcaseOnly(graph({ [`${DESIGN}/DiscoverCard.tsx`]: { importers: [ENTRY, SHOP], exports: ['DiscoverCard'] } }))
    expect(only(`${DESIGN}/DiscoverCard.tsx`)).toBe(false)
  })

  it('should leave design modules that import each other in a cycle in place', () => {
    const only = showcaseOnly(
      graph({
        [`${DESIGN}/A.tsx`]: { importers: [ENTRY, `${DESIGN}/B.tsx`], exports: ['A'] },
        [`${DESIGN}/B.tsx`]: { importers: [ENTRY, `${DESIGN}/A.tsx`], exports: ['B'] }
      })
    )
    expect(only(`${DESIGN}/A.tsx`)).toBe(false)
    expect(only(`${DESIGN}/B.tsx`)).toBe(false)
  })

  it('should leave a module nothing imports in place', () => {
    expect(showcaseOnly(graph({ [`${DESIGN}/Orphan.tsx`]: { exports: ['Orphan'] } }))(`${DESIGN}/Orphan.tsx`)).toBe(false)
  })
})

describe('when the build tree-shakes the design modules', () => {
  it('should mark the design modules free of import effects, and leave the Showcase, styles and features to the default', () => {
    expect(designSideEffects(`${DESIGN}/DiscoverCard.tsx`)).toBe(false)
    expect(designSideEffects(`${DESIGN}/rarityArt.ts`)).toBe(false)
    for (const id of [SHOWCASE, `${DESIGN}/ShowcaseIndex.ts`, `${DESIGN}/DiscoverCard.module.css`, SHOP])
      expect(designSideEffects(id)).toBeUndefined()
  })
})
