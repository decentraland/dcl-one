// The in-world Shop's UI (library/ui/apps/shop/ui, docs/ui-standards.md, App-specific UI): it may import React,
// the HUD's design primitives and assets, its own files and assets, the public @dcl/one-ui components, the
// Shop's pure model and formatting (library/shop) and, from its host (@shop-host), only the credits mark,
// never data (queries, fetchers, TanStack) nor the HUD's runtime (session, engine, menu, its libs). It runs
// here, with the /play tests, because /play is the UI's host.

import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

const UI = join(__dirname, '../../../../../library/ui/apps/shop/ui')
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
    const names = [...readFileSync(join(UI, file), 'utf8').matchAll(/import\s+(?:type\s+)?\{([^}]*)\}\s+from\s+['"]@shop-host['"]/g)].flatMap((m) => m[1].split(',').map((n) => n.trim()).filter(Boolean))
    expect(names.filter((n) => n !== 'CreditMark')).toEqual([])
  })

  it.each(files)('%s imports no data and nothing of the HUD runtime', (file) => {
    const outside = specifiers(readFileSync(join(UI, file), 'utf8')).filter((s) => !ALLOWED.some((a) => a.test(s)))
    expect(outside).toEqual([])
  })
})
