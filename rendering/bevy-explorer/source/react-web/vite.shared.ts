import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import type { Alias } from 'vite'

// Paths from this file's directory. Not `new URL(path, import.meta.url)`: Vite rewrites that pattern into an asset
// URL in the modules it transforms, and tests import this file.
const here = dirname(fileURLToPath(import.meta.url))

// Sources the HUD compiles from library/: the in-world Shop's logic, the catalyst's DTOs and the shared UI
// foundations. library/ has no node_modules, so the packages they import resolve to this app's copies
// (`SHARED_DEDUPE`), and the bundle holds one viem, zod, React and React Query. tsconfig.json maps the same
// paths for the type check. tooling/play-artifact.py hashes these trees with the HUD (`HUD_SHARED`); a test
// keeps the lists together.
export const SHARED_SOURCES: Record<string, string> = {
  '@shop': resolve(here, '../../../../library/shop'),
  '@catalyst-api': resolve(here, '../../../../library/catalyst-api'),
  // @dcl/one-ui's public entries (library/ui/package.json "exports"). The subpaths come first: Vite takes the
  // first alias that matches, and the bare name also matches any subpath.
  '@dcl/one-ui/foundations.css': resolve(here, '../../../../library/ui/foundations/foundations.css'),
  '@dcl/one-ui/text-styles.module.css': resolve(here, '../../../../library/ui/foundations/typography/text-styles.module.css'),
  '@dcl/one-ui': resolve(here, '../../../../library/ui/index.ts')
}

// What the dev server may serve from outside this app: the trees behind those entries.
export const SHARED_TREES = [
  SHARED_SOURCES['@shop'],
  SHARED_SOURCES['@catalyst-api'],
  SHARED_SOURCES['@dcl/one-ui'],
  resolve(here, '../../../../library/ui/foundations')
]

export const SHARED_DEDUPE = ['react', 'react-dom', 'viem', 'zod', '@tanstack/react-query']

// library/ui registers its Inter faces (foundations/typography/inter.css) from Git LFS copies of
// @fontsource-variable/inter 5.3.0, and the HUD's builds don't fetch LFS. Those faces take this app's copy of
// the same package instead: the same bytes (src/test/sharedUi.test.tsx compares them with the digests library/ui
// pins), so the bundle holds one file per face, shared with the 'Inter Variable' faces main.tsx loads. Vite
// resolves a stylesheet's url() through its aliases only, never a plugin, so this is an alias; inter.css is the
// only source that names these files (the same test checks it).
export const INTER_FILES = resolve(here, 'node_modules/@fontsource-variable/inter/files')

export const SHARED_FONTS: Alias = {
  find: /^\.\/fonts\/(inter-[a-z-]+-wght-normal\.woff2)$/,
  replacement: `${INTER_FILES}/$1`
}

export const SHARED_ALIASES: Alias[] = [
  SHARED_FONTS,
  ...Object.entries(SHARED_SOURCES).map(([find, replacement]) => ({ find, replacement }))
]
