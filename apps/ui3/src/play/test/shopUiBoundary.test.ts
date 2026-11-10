// The in-world Shop's UI (features/shop/ui) moves to library/ui/apps/shop/ once that exists
// (docs/ui-standards.md, App-specific UI): it may import React, the HUD's design primitives and assets,
// its own files, the Shop's pure model and formatting (library/shop) and the web Shop's credits mark —
// never data (queries, fetchers, TanStack) nor the HUD's runtime (session, engine, menu, its libs).

import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

const UI = join(__dirname, '../features/shop/ui')
const ALLOWED = [
  /^react$/,
  /^\.\/[A-Za-z]+(\.module\.css)?$/,
  /^\.\.\/\.\.\/\.\.\/design$/,
  /^\.\.\/\.\.\/\.\.\/assets\//,
  /^\.\.\/\.\.\/\.\.\/\.\.\/shop\/credits\/components\/CreditMark$/,
  /^@shop\/(model\/|checkout\/lib\/currency$|discovery\/cardPrice$|discovery\/lib\/price-slider$)/
]

// `import x from '…'`, `import '…'` and `export … from '…'`, across lines.
const specifiers = (source: string): string[] =>
  [...source.matchAll(/^\s*import\s+(?:[^'";]*?\sfrom\s+)?['"]([^'"]+)['"]|^\s*export\s+[^'";]*?\sfrom\s+['"]([^'"]+)['"]/gm)].map((m) => m[1] ?? m[2])

describe('the in-world Shop UI', () => {
  const files = readdirSync(UI).filter((f) => /\.tsx?$/.test(f))

  it('has its components', () => {
    expect(files).toEqual(expect.arrayContaining(['index.ts', 'ShopItemCard.tsx', 'ShopItemPanel.tsx', 'ShopFilters.tsx', 'ShopHome.tsx']))
  })

  it.each(files)('%s imports no data and nothing of the HUD runtime', (file) => {
    const outside = specifiers(readFileSync(join(UI, file), 'utf8')).filter((s) => !ALLOWED.some((a) => a.test(s)))
    expect(outside).toEqual([])
  })
})
