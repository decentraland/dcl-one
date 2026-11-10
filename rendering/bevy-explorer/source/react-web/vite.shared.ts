import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import type { Alias } from 'vite'
import { readSharedPaths } from './scripts/shared-inputs.mjs'

// Paths from this file's directory. Not `new URL(path, import.meta.url)`: Vite rewrites that pattern into an asset
// URL in the modules it transforms, and tests import this file.
const here = dirname(fileURLToPath(import.meta.url))

// Sources the HUD compiles from library/: the in-world Shop's logic, the catalyst's DTOs, the shared UI
// foundations, the HUD UI itself (library/ui/apps/hud) and the Explorer page's core (library/explorer-core),
// both shared with the Sites /play host. library/ has no node_modules, so the packages they import resolve to this app's copies
// (`SHARED_DEDUPE`), and the bundle holds one viem, zod, React and React Query. tsconfig.json maps the same
// paths for the type check. The computed shared-inputs.json contract records the actual compiler,
// web/native and test graph closure for source selection, fingerprints, dev access, Nix and native builds.
export const SHARED_SOURCES: Record<string, string> = {
  '@shop': resolve(here, '../../../../library/shop'),
  '@catalyst-api': resolve(here, '../../../../library/catalyst-api'),
  // Public package entries match library/ui/package.json "exports". Keep the longer CSS
  // subpath before the foundations entry because Vite uses the first matching alias.
  '@dcl/one-ui/foundations.css': resolve(here, '../../../../library/ui/foundations/foundations.css'),
  '@dcl/one-ui/text-styles.module.css': resolve(here, '../../../../library/ui/foundations/typography/text-styles.module.css'),
  '@dcl/one-ui/foundations': resolve(here, '../../../../library/ui/foundations/index.ts'),
  '@hud': resolve(here, '../../../../library/ui/apps/hud'),
  // The Explorer page's core both HUD hosts share: engine contract, drivers, sign-in, server reads.
  '@explorer-core': resolve(here, '../../../../library/explorer-core'),
  // The behaviours this host supplies to the shared HUD (library/explorer-core/hostContract.ts).
  '@hud-host': resolve(here, 'src/host.ts')
}

// What the dev server may serve from outside this app: the trees behind those entries.
// The folders holding them too: Vite matches an allowed file exactly, so an asset imported with a query
// (the shared HUD's `create.svg?no-inline`) is only served from an allowed folder.
const SHARED_FILES = readSharedPaths(resolve(here, '../../../..'), { mode: 'config' })
export const SHARED_TREES = [...SHARED_FILES, ...new Set(SHARED_FILES.map((file) => dirname(file)))]

// The shared HUD's tests (library/ui/apps/hud, library/explorer-core) import the testing packages too, and the
// shared-inputs graph walks them with both configs: they resolve to this app's copies like the rest. The app
// itself never imports them.
const TEST_PACKAGES = ['@testing-library/react', '@testing-library/user-event', '@testing-library/dom', '@testing-library/jest-dom']
export const SHARED_DEDUPE = ['react', 'react-dom', 'viem', 'zod', '@tanstack/react-query', ...TEST_PACKAGES]

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

// `vitest` by its exact name, for the same tests (its subpaths keep their own resolution).
const VITEST: Alias = { find: /^vitest$/, replacement: resolve(here, 'node_modules/vitest/dist/index.js') }

// The @noble modules library/explorer-core imports, by their exact names: a dependency of this app keeps an
// older @noble of its own, which a dedupe would break. tsconfig.json maps all of @noble/* for the type check.
const NOBLE: Alias[] = ['curves/secp256k1.js', 'hashes/sha3.js', 'hashes/utils.js'].map((path) => ({
  find: '@noble/' + path,
  replacement: resolve(here, 'node_modules/@noble/' + path)
}))

export const SHARED_ALIASES: Alias[] = [
  SHARED_FONTS,
  VITEST,
  ...NOBLE,
  ...Object.entries(SHARED_SOURCES).map(([find, replacement]) => ({ find, replacement }))
]
