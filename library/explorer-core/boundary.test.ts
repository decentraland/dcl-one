// library/explorer-core is the Explorer page's core: the engine contract (protocol and generated
// bindings), the drivers, sign-in, shell messaging, analytics and the server reads both HUD hosts
// share. The HUD UI (library/ui/apps/hud) is built on it, never the other way round. What this file
// holds it to:
//
// - it is TypeScript modules only: no components, styles or assets;
// - its modules import only the core itself, the host module (`@hud-host`) and packages; never the HUD
//   UI (`@hud/*` or library/ui), and never React or a React-only package.

import { readdirSync, readFileSync, statSync } from 'node:fs'
import { dirname, join, relative, resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const ROOT = __dirname

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    return statSync(path).isDirectory() ? walk(path) : [relative(ROOT, path)]
  })
}

const all = walk(ROOT)
const isTest = (file: string): boolean => /\.test\.tsx?$/.test(file)
const modules = all.filter((file) => !isTest(file) && file !== 'README.md')

// Static imports and re-exports, dynamic imports and require, read from the raw source: a
// commented-out import counts too, which can only fail the check.
function specifiers(source: string): string[] {
  return [
    ...source.matchAll(/^\s*(?:import|export)\s+(?:[^'";]*?\sfrom\s+)?['"]([^'"]+)['"]/gm),
    ...source.matchAll(/\bimport\(\s*['"]([^'"]+)['"]\s*\)/g),
    ...source.matchAll(/\brequire\(\s*['"]([^'"]+)['"]\s*\)/g)
  ].map((m) => m[1] as string)
}

const REACT = /^(react|react-dom|@tanstack\/react-query|@testing-library\/react)(\/|$)/

describe('library/explorer-core', () => {
  it('holds TypeScript modules only', () => {
    expect(modules.filter((file) => !file.endsWith('.ts'))).toEqual([])
  })

  it.each(modules)('%s imports no HUD UI and no React', (file) => {
    const wrong = specifiers(readFileSync(join(ROOT, file), 'utf8')).filter((spec) => {
      if (spec.startsWith('.')) return !resolve(dirname(join(ROOT, file)), spec).startsWith(ROOT + '/')
      return spec.startsWith('@hud/') || spec.includes('library/ui') || REACT.test(spec)
    })
    expect(wrong).toEqual([])
  })
})
