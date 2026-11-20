// The in-world Shop's UI (library/ui/apps/shop/ui, docs/ui-standards.md, App-specific UI): it may import React,
// the HUD's design primitives and assets, its own files and assets, the public @dcl/one-ui components, the
// Shop's pure model and formatting (library/shop) and, from its host (@shop-host), only the credits mark,
// never data (queries, fetchers, TanStack) nor the HUD's runtime (session, engine, menu, its libs). It runs
// here, with the /play tests, because /play is the UI's host.

import { readdirSync, readFileSync } from 'node:fs'
import { dirname, join, relative, resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const LIBRARY = resolve(__dirname, '../../../../../library')
const SHOP = join(LIBRARY, 'ui/apps/shop')
const UI = join(SHOP, 'ui')
const ALLOWED = [
  /^react$/,
  // The public components (the item card and its skeleton are the design system's ShopCard).
  /^@dcl\/one-ui$/,
  /^\.\/[A-Za-z]+(\.module\.css)?$/,
  /^@hud\/design$/,
  /^@hud\/assets\//,
  // The in-world Shop's own assets, beside it.
  /^\.\.\/assets\//,
  // Its host's credits mark (the test below holds it to that one name).
  /^@shop-host$/,
  /^@shop\/(model\/|checkout\/lib\/currency$|discovery\/cardPrice$|discovery\/lib\/price-slider$)/
]

// `import x from '…'`, `import '…'` and `export … from '…'`, across lines.
const specifiers = (source: string): string[] =>
  [...source.matchAll(/^\s*import\s+(?:[^'";]*?\sfrom\s+)?['"]([^'"]+)['"]|^\s*export\s+[^'";]*?\sfrom\s+['"]([^'"]+)['"]/gm)].map((m) => m[1] ?? m[2])

describe('the in-world Shop UI', () => {
  const files = readdirSync(UI).filter((f) => /\.tsx?$/.test(f))

  it('has its components', () => {
    expect(files).toEqual(expect.arrayContaining(['index.ts', 'ShopItemPanel.tsx', 'ShopFilters.tsx', 'ShopHome.tsx']))
  })

  it.each(files)('%s takes only the credits mark from its host', (file) => {
    const source = readFileSync(join(UI, file), 'utf8')
    // Every clause that names @shop-host: a named import of CreditMark alone passes; a namespace or default
    // import, a re-export or any other name does not.
    const clauses = [...source.matchAll(/^\s*(import|export)\s+([^'";]*?)\s+from\s+['"]@shop-host['"]/gm)].map((m) => ({ kind: m[1], what: m[2].trim() }))
    for (const { kind, what } of clauses) {
      expect(kind).toBe('import')
      expect(what).toMatch(/^(type\s+)?\{[^}]*\}$/)
      const names = what.replace(/^type\s+/, '').slice(1, -1).split(',').map((n) => n.trim()).filter(Boolean)
      expect(names).toEqual(['CreditMark'])
    }
  })

  it.each(files)('%s imports no data and nothing of the HUD runtime', (file) => {
    const outside = specifiers(readFileSync(join(UI, file), 'utf8')).filter((s) => !ALLOWED.some((a) => a.test(s)))
    expect(outside).toEqual([])
  })
})

// Every module of the in-world Shop, its data and React state included: `library/` depends only on library
// code (AGENTS.md), so nothing reaches into an app, by a relative path or by the apps' `@ui` alias.
const sources = (dir: string): string[] =>
  readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name)
    if (entry.isDirectory()) return entry.name === 'assets' ? [] : sources(path)
    return /\.tsx?$/.test(entry.name) ? [path] : []
  })
// Static imports and re-exports (as above) and dynamic `import('…')`.
const allSpecifiers = (source: string): string[] => [
  ...specifiers(source),
  ...[...source.matchAll(/\bimport\(\s*['"]([^'"]+)['"]\s*\)/g)].map((m) => m[1])
]

/** Whether a specifier, written in `fromDir`, leaves library/: a relative path out of it, or the `@ui` alias. */
const leavesLibrary = (fromDir: string) => (specifier: string): boolean =>
  specifier.startsWith('.') ? relative(LIBRARY, resolve(fromDir, specifier)).startsWith('..') : /^@ui\//.test(specifier)

describe('the in-world Shop, every module', () => {
  const files = sources(SHOP).map((path) => relative(SHOP, path))

  it('walks its data modules and its UI', () => {
    expect(files).toEqual(expect.arrayContaining(['InWorldShop.tsx', 'catalogueQuery.ts', 'shopProtocol.ts', join('ui', 'ShopHome.tsx')]))
  })

  it.each(files)('%s imports nothing outside library/', (file) => {
    const outside = allSpecifiers(readFileSync(join(SHOP, file), 'utf8')).filter(leavesLibrary(dirname(join(SHOP, file))))
    expect(outside).toEqual([])
  })

  it('catches a relative import into an app and the apps\' alias, static or dynamic', () => {
    const offending = [
      "import { shopClient } from '../../../../apps/ui3/src/shop/api/host'",
      "import { x } from '@ui/shop/api/host'",
      "const m = import('../../../../apps/ui3/src/shop/api/host')"
    ].join('\n')
    const found = allSpecifiers(offending).filter(leavesLibrary(SHOP))
    expect(found).toHaveLength(3)
  })
})

