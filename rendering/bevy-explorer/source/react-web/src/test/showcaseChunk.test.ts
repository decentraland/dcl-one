import { describe, expect, it } from 'vitest'
import { statSync } from 'node:fs'
import { DESIGN_ROOTS, designSideEffects, isMenu, isShowcase, showcaseInEagerGraph, showcaseOnly } from '../../vite.config'

// The build's chunking of the design modules: what only the dev-only Showcase uses stays out of the eager chunks.
// The shared HUD's design folder, as the bundler names its modules.
const DESIGN = DESIGN_ROOTS.find((root) => root.endsWith('/apps/hud/design/'))!.slice(0, -1)
const ENTRY = `${DESIGN}/index.ts`
const SHOWCASE = `${DESIGN}/Showcase.tsx`
const SHOP = `${DESIGN.slice(0, -'design'.length)}features/shop/ShopPage.tsx`

type Module = { importers: string[]; exports: string[]; code: string | null }

// A design entry re-exporting LogoPattern, which only the Showcase names, and Button, which the Shop names too.
function graph(changes: Record<string, Partial<Module>> = {}) {
  const modules: Record<string, Partial<Module>> = {
    [ENTRY]: { importers: [SHOWCASE, SHOP] },
    [SHOWCASE]: { code: 'import { Button, LogoPattern } from "."' },
    [SHOP]: { code: 'import { Button } from "../../design"; jsx(Button, {})' },
    [`${DESIGN}/LogoPattern.tsx`]: { importers: [ENTRY], exports: ['LogoPattern'] },
    [`${DESIGN}/LogoPattern.module.css`]: { importers: [`${DESIGN}/LogoPattern.tsx`], exports: ['default'] },
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
    expect(only(`${DESIGN}/LogoPattern.tsx`)).toBe(true)
    expect(only(`${DESIGN}/LogoPattern.module.css`)).toBe(true)
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
    expect(only(`${DESIGN}/LogoPattern.tsx`)).toBe(false)
  })

  it('should leave a primitive a module outside design/ imports directly in place', () => {
    const only = showcaseOnly(graph({ [`${DESIGN}/LogoPattern.tsx`]: { importers: [ENTRY, SHOP], exports: ['LogoPattern'] } }))
    expect(only(`${DESIGN}/LogoPattern.tsx`)).toBe(false)
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
    expect(designSideEffects(`${DESIGN}/LogoPattern.tsx`)).toBe(false)
    expect(designSideEffects(`${DESIGN}/rarityArt.ts`)).toBe(false)
    for (const id of [SHOWCASE, `${DESIGN}/ShowcaseIndex.ts`, `${DESIGN}/LogoPattern.module.css`, SHOP])
      expect(designSideEffects(id)).toBeUndefined()
  })

  it('should list only design folders that exist: the shared HUD', () => {
    expect(DESIGN_ROOTS.map((root) => root.split('/').slice(-4).join('/'))).toEqual(['apps/hud/design/'])
    for (const root of DESIGN_ROOTS) expect(statSync(root).isDirectory(), root).toBe(true)
  })

  it('should leave a design folder in a dependency, or one only named like an owned folder, to the default', () => {
    for (const id of [
      `${DESIGN.slice(0, -'design'.length)}node_modules/some-pkg/src/design/Widget.tsx`,
      '/elsewhere/apps/hud/design/Button.tsx',
      '/elsewhere/explorer-core/design/index.ts'
    ]) {
      expect(designSideEffects(id)).toBeUndefined()
      expect(showcaseOnly(graph())(id)).toBe(false)
    }
  })

  it('should put only the menu pages of the shared HUD and the Explorer page core in the menus chunk', () => {
    const hud = DESIGN.slice(0, -'design'.length)
    const core = `${hud.slice(0, -'ui/apps/hud/'.length)}explorer-core/`
    expect(isMenu(`${hud}features/map/MapPage.tsx`)).toBe(true)
    expect(isMenu(`${core}features/places/placesApi.ts`)).toBe(true)
    for (const id of [
      `${hud}features/shop/ShopPage.tsx`,
      `${hud}node_modules/some-pkg/src/features/map/index.js`,
      '/elsewhere/apps/hud/features/map/MapPage.tsx',
      '/elsewhere/explorer-core/features/places/placesApi.ts'
    ])
      expect(isMenu(id), id).toBe(false)
  })

  it('should tell the Showcase by its design module name, not by a path that merely contains the word', () => {
    expect(isShowcase(SHOWCASE)).toBe(true)
    expect(isShowcase(`${DESIGN}/ShowcaseIndex.ts`)).toBe(true)
    expect(isShowcase(`${DESIGN}/Button.tsx`)).toBe(false)
    expect(isShowcase('/home/Showcase/checkout/library/ui/apps/hud/features/shop/ShopPage.tsx')).toBe(false)
    expect(isShowcase('/elsewhere/apps/hud/design/Showcase.tsx')).toBe(false)
  })
})

type Chunk = { type: 'chunk'; fileName: string; name: string; isEntry: boolean; facadeModuleId: string | null; imports: string[]; moduleIds: string[] }
const chunk = (fileName: string, changes: Partial<Chunk> = {}): Chunk => ({
  type: 'chunk',
  fileName,
  name: fileName.replace(/^assets\/|-.*$/g, ''),
  isEntry: false,
  facadeModuleId: null,
  imports: [],
  moduleIds: [],
  ...changes
})

// app.html's entry imports the design chunk; the Showcase sits behind a lazy import.
function bundle(changes: { entryImports?: string[]; html?: string | Uint8Array; entry?: Partial<Chunk> } = {}) {
  const files = [
    chunk('assets/app-1.js', {
      isEntry: true,
      facadeModuleId: '/repo/react-web/app.html',
      imports: changes.entryImports ?? ['assets/design-1.js'],
      ...changes.entry
    }),
    chunk('assets/design-1.js', { moduleIds: [ENTRY, `${DESIGN}/Button.tsx`] }),
    chunk('assets/Showcase-1.js', { moduleIds: [SHOWCASE, `${DESIGN}/LogoPattern.tsx`] }),
    { type: 'asset' as const, fileName: 'app.html', source: changes.html ?? '<script src="./assets/app-1.js"></script>' }
  ]
  return Object.fromEntries(files.map((file) => [file.fileName, file]))
}

describe('when the build checks what app.html loads eagerly', () => {
  it('should pass a bundle whose Showcase is reached only by a lazy import', () => {
    expect(showcaseInEagerGraph(bundle(), 'app.html')).toEqual([])
  })

  it('should fail a bundle whose entry reaches the Showcase chunk by static imports', () => {
    const imports = bundle({ entryImports: ['assets/design-1.js', 'assets/Showcase-1.js'] })
    expect(showcaseInEagerGraph(imports, 'app.html')).toEqual([
      'app.html statically imports assets/Showcase-1.js, which holds the Showcase'
    ])
  })

  it('should fail a bundle whose eager chunk holds the Showcase module under another name', () => {
    const merged = bundle()
    merged['assets/design-1.js'] = chunk('assets/design-1.js', { moduleIds: [ENTRY, SHOWCASE] })
    expect(showcaseInEagerGraph(merged, 'app.html')).toEqual([
      'app.html statically imports assets/design-1.js, which holds the Showcase'
    ])
  })

  // The bundler may hand the page over as a string or as UTF-8 bytes.
  const preload = '<script src="./assets/app-1.js"></script><link rel="modulepreload" href="./assets/Showcase-1.js">'
  it.each([
    ['a string', preload],
    ['UTF-8 bytes', new TextEncoder().encode(preload)]
  ])('should fail a page that preloads the Showcase chunk, given as %s', (_, html) => {
    expect(showcaseInEagerGraph(bundle({ html }), 'app.html')).toEqual([
      'app.html references assets/Showcase-1.js, which holds the Showcase'
    ])
  })

  it('should fail a bundle with no entry for the page instead of checking nothing', () => {
    expect(showcaseInEagerGraph(bundle({ entry: { isEntry: false } }), 'app.html')).toEqual([
      'the bundle has no entry chunk for app.html, so its imports cannot be checked'
    ])
  })
})
